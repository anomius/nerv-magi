//! herdr collector: one short connection per request (newline-delimited JSON) plus one long-lived
//! `events.subscribe` connection for pushed agent status changes.
//!
//! Read-only towards herdr: the only methods ever called are `session.snapshot`, `agent.list`,
//! `pane.process_info` and `events.subscribe`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};

use crate::state::{Agent, AgentMeta, AgentStatus, HerdrLink, PaneJob, Update, Workspace};

/// Periodic re-snapshot + pass (RSS, agent cwd/branch, pane jobs); catches renames and anything
/// the events missed.
const PERIOD: Duration = Duration::from_secs(10);
/// Topology events come in bursts; re-snapshot once the burst has settled.
const TOPOLOGY_SETTLE: Duration = Duration::from_millis(150);
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const BACKOFF: [Duration; 3] = [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(5)];
const TOPOLOGY_EVENTS: [&str; 7] = [
    "pane.created",
    "pane.closed",
    "pane.agent_detected",
    "workspace.created",
    "workspace.closed",
    "workspace.renamed",
    "tab.renamed",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Status { pane_id: String, status: AgentStatus, title: Option<String> },
    /// Something in the layout changed: re-snapshot.
    Topology,
}

/// One event-stream line. Acks, unknown kinds and garbage are `None`.
pub fn parse_event(line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line).ok()?;
    // herdr 0.9.3 sends status kinds dotted (`pane.agent_status_changed`) and topology kinds
    // underscored (`pane_closed`); compare in one spelling.
    let kind = v.get("event")?.as_str()?.replace('.', "_");
    let data = v.get("data")?;
    if kind == "pane_agent_status_changed" {
        return Some(Event::Status {
            pane_id: data.get("pane_id")?.as_str()?.to_string(),
            status: AgentStatus::parse(data.get("agent_status")?.as_str()?),
            title: data.get("title").and_then(Value::as_str).map(str::to_string),
        });
    }
    TOPOLOGY_EVENTS.iter().any(|t| t.replace('.', "_") == kind).then_some(Event::Topology)
}

/// Agents (from `agent.list`, which carries `name`) joined with workspace and tab labels from
/// `session.snapshot`. Both arguments are the responses' `result` objects.
pub fn agents_from(snapshot: &Value, agent_list: &Value, now: SystemTime) -> (Vec<Agent>, Vec<Workspace>) {
    let snap = &snapshot["snapshot"];
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let items = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let workspaces: Vec<Workspace> = items(&snap["workspaces"])
        .iter()
        .map(|w| Workspace {
            id: text(w, "workspace_id"),
            label: text(w, "label"),
            number: w.get("number").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
        .collect();
    let tabs: HashMap<String, String> = items(&snap["tabs"]).iter().map(|t| (text(t, "tab_id"), text(t, "label"))).collect();
    let agents = items(&agent_list["agents"])
        .iter()
        .filter_map(|a| {
            let pane_id = a.get("pane_id")?.as_str()?.to_string();
            let workspace_id = text(a, "workspace_id");
            let workspace_label =
                workspaces.iter().find(|w| w.id == workspace_id).map_or_else(|| workspace_id.clone(), |w| w.label.clone());
            Some(Agent {
                pane_id,
                name: a.get("name").and_then(Value::as_str).map(str::to_string),
                kind: a.get("agent").and_then(Value::as_str).unwrap_or("agent").to_string(),
                status: AgentStatus::parse(a.get("agent_status").and_then(Value::as_str).unwrap_or("")),
                since: now,
                workspace_id,
                workspace_label,
                tab_label: tabs.get(&text(a, "tab_id")).cloned().unwrap_or_default(),
                title: text(a, "terminal_title_stripped"),
                rss_bytes: None,
            })
        })
        .collect();
    (agents, workspaces)
}

/// Foreground processes with these names are a shell prompt, not a job.
const SHELLS: [&str; 5] = ["bash", "zsh", "sh", "fish", "nu"];

fn arr(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}

/// `$HOME` prefix → `~` (only at a path boundary).
pub fn tilde(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return path.to_string();
    }
    match path.strip_prefix(home) {
        Some("") => "~".into(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// Branch checked out at `dir`: the nearest ancestor with a usable `.git` (a directory, or a
/// worktree's `gitdir: <path>` file), then its `HEAD`. Like git, a `.git` without a readable
/// `HEAD` (e.g. an empty leftover directory) is skipped. Detached HEAD or no repo → None.
pub fn git_branch(dir: &Path) -> Option<String> {
    for d in dir.ancestors() {
        let dot = d.join(".git");
        let gitdir = if dot.is_dir() {
            dot
        } else {
            let text = fs::read_to_string(&dot).unwrap_or_default();
            let Some(target) = text.trim().strip_prefix("gitdir:") else { continue };
            d.join(target.trim()) // absolute targets replace `d`
        };
        let Ok(head) = fs::read_to_string(gitdir.join("HEAD")) else { continue };
        return head.trim().strip_prefix("ref: refs/heads/").map(str::to_string);
    }
    None
}

/// cwd (`foreground_cwd`, else `cwd`; `~`-shortened), git branch and session id of one
/// `agent.list` entry.
pub fn meta_from(agent: &Value, home: &str) -> AgentMeta {
    let text = |k: &str| agent.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    let cwd = text("foreground_cwd").or_else(|| text("cwd")).unwrap_or("");
    AgentMeta {
        branch: (!cwd.is_empty()).then(|| git_branch(Path::new(cwd))).flatten(),
        cwd: tilde(cwd, home),
        session_id: agent["agent_session"]["value"].as_str().map(str::to_string),
    }
}

/// Non-agent panes of a `session.snapshot` result, as `PaneJob`s still missing `command`
/// (`cwd` holds the pane's raw cwd until `pane.process_info` gives the job's own).
pub fn job_panes(snapshot: &Value, agent_panes: &BTreeSet<String>) -> Vec<PaneJob> {
    let snap = &snapshot["snapshot"];
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let label_of = |list: &str, id_key: &str, id: &str| {
        arr(&snap[list]).iter().find(|x| x[id_key] == id).map_or_else(|| id.to_string(), |x| text(x, "label"))
    };
    arr(&snap["panes"])
        .iter()
        .filter_map(|p| {
            let pane_id = p.get("pane_id")?.as_str()?;
            if agent_panes.contains(pane_id) {
                return None;
            }
            let cwd = [text(p, "foreground_cwd"), text(p, "cwd")].into_iter().find(|c| !c.is_empty()).unwrap_or_default();
            Some(PaneJob {
                pane_id: pane_id.to_string(),
                workspace_label: label_of("workspaces", "workspace_id", &text(p, "workspace_id")),
                tab_label: label_of("tabs", "tab_id", &text(p, "tab_id")),
                label: p.get("label").and_then(Value::as_str).filter(|l| !l.is_empty()).unwrap_or(pane_id).to_string(),
                command: String::new(),
                cwd,
            })
        })
        .collect()
}

/// (command, cwd) of the first foreground process that is not a shell: `basename(argv[0])` and
/// the rest of argv. A pane sitting at a shell prompt → None.
pub fn job_from(process_info: &Value) -> Option<(String, String)> {
    let procs = arr(&process_info["process_info"]["foreground_processes"]);
    let p = procs.iter().find(|p| p["name"].as_str().is_some_and(|n| !SHELLS.contains(&n.trim_start_matches('-'))))?;
    let argv: Vec<&str> = arr(&p["argv"]).iter().filter_map(Value::as_str).collect();
    let command = match argv.split_first() {
        Some((first, rest)) => {
            let base = Path::new(first).file_name().and_then(|b| b.to_str()).unwrap_or(first);
            std::iter::once(base).chain(rest.iter().copied()).collect::<Vec<_>>().join(" ")
        }
        None => p["name"].as_str()?.to_string(),
    };
    Some((command, p["cwd"].as_str().unwrap_or("").to_string()))
}

/// ppid from `/proc/<pid>/stat`: the 2nd field after the **last** `)` (comm may hold spaces and parens).
pub fn stat_ppid(stat: &str) -> Option<i32> {
    stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()
}

fn vm_rss_bytes(status: &str) -> u64 {
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map_or(0, |kib| kib * 1024)
}

/// Parent → children, from one scan of a proc root; shared by every agent in an RSS pass.
struct ProcTree {
    root: PathBuf,
    children: HashMap<i32, Vec<i32>>,
}

impl ProcTree {
    fn scan(proc_root: &Path) -> Self {
        let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
        for entry in fs::read_dir(proc_root).into_iter().flatten().flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else { continue };
            if let Some(ppid) = fs::read_to_string(entry.path().join("stat")).ok().as_deref().and_then(stat_ppid) {
                children.entry(ppid).or_default().push(pid);
            }
        }
        Self { root: proc_root.to_path_buf(), children }
    }

    /// Resident bytes of `pid` and all its descendants; 0 when `pid` does not exist.
    fn rss(&self, pid: i32) -> u64 {
        let (mut total, mut stack, mut seen) = (0, vec![pid], HashSet::new());
        while let Some(p) = stack.pop() {
            if !seen.insert(p) {
                continue;
            }
            let Ok(status) = fs::read_to_string(self.root.join(p.to_string()).join("status")) else { continue };
            total += vm_rss_bytes(&status);
            stack.extend(self.children.get(&p).into_iter().flatten());
        }
        total
    }
}

/// Single-tree convenience; the collector shares one `ProcTree` scan across all agents.
#[allow(dead_code)]
pub fn tree_rss(proc_root: &Path, pid: i32) -> u64 {
    ProcTree::scan(proc_root).rss(pid)
}

/// `events.subscribe` params: status changes for each agent pane (herdr requires a `pane_id`
/// per entry) plus the topology kinds that should trigger a re-snapshot.
pub fn subscriptions(agent_panes: &BTreeSet<String>) -> Value {
    let status = agent_panes.iter().map(|p| json!({"type": "pane.agent_status_changed", "pane_id": p}));
    let topology = TOPOLOGY_EVENTS.iter().map(|t| json!({"type": t}));
    json!({ "subscriptions": status.chain(topology).collect::<Vec<_>>() })
}

/// A response line → its `result`, or the server's error message.
fn response(line: &str) -> Result<Value, String> {
    if line.is_empty() {
        return Err("connection closed".into());
    }
    let mut v: Value = serde_json::from_str(line).map_err(|e| format!("bad response: {e}"))?;
    if let Some(e) = v.get("error") {
        return Err(e.get("message").and_then(Value::as_str).unwrap_or("error").to_string());
    }
    v.get_mut("result").map(Value::take).ok_or_else(|| "response without result".into())
}

fn connect(path: &Path, method: &str, params: Value) -> Result<UnixStream, String> {
    let mut s = UnixStream::connect(path).map_err(|e| format!("{}: {e}", path.display()))?;
    s.set_write_timeout(Some(CALL_TIMEOUT)).ok();
    writeln!(s, "{}", json!({"id": "nerv", "method": method, "params": params})).map_err(|e| format!("{method}: {e}"))?;
    Ok(s)
}

/// One request on a fresh connection.
pub(crate) fn call(path: &Path, method: &str, params: Value) -> Result<Value, String> {
    let s = connect(path, method, params)?;
    s.set_read_timeout(Some(CALL_TIMEOUT)).ok();
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).map_err(|e| format!("{method}: {e}"))?;
    response(&line).map_err(|e| format!("{method}: {e}"))
}

/// One snapshot: what the UI gets now, plus what the next pass needs.
struct Snap {
    agents: Vec<Agent>,
    workspaces: Vec<Workspace>,
    meta: Vec<(String, AgentMeta)>,
    job_panes: Vec<PaneJob>,
}

fn snapshot(path: &Path, home: &str) -> Result<Snap, String> {
    // The CLI calls this `herdr api snapshot`; on the socket the method is `session.snapshot`.
    let snap = call(path, "session.snapshot", json!({}))?;
    let list = call(path, "agent.list", json!({}))?;
    let (agents, workspaces) = agents_from(&snap, &list, SystemTime::now());
    let meta = arr(&list["agents"])
        .iter()
        .filter_map(|a| Some((a.get("pane_id")?.as_str()?.to_string(), meta_from(a, home))))
        .collect();
    let job_panes = job_panes(&snap, &pane_set(&agents));
    Ok(Snap { agents, workspaces, meta, job_panes })
}

fn pane_set(agents: &[Agent]) -> BTreeSet<String> {
    agents.iter().map(|a| a.pane_id.clone()).collect()
}

/// Opens the event stream and waits for its ack.
fn subscribe(path: &Path, panes: &BTreeSet<String>) -> Result<BufReader<UnixStream>, String> {
    let s = connect(path, "events.subscribe", subscriptions(panes))?;
    s.set_read_timeout(Some(CALL_TIMEOUT)).ok();
    let mut events = BufReader::new(s);
    let mut ack = String::new();
    events.read_line(&mut ack).map_err(|e| format!("events.subscribe: {e}"))?;
    response(&ack).map_err(|e| format!("events.subscribe: {e}"))?;
    Ok(events)
}

/// RSS of each agent pane's process tree (rooted at the pane's shell). Panes herdr can't describe
/// any more are skipped; the next snapshot drops them.
fn rss_rows(path: &Path, panes: &BTreeSet<String>) -> Vec<(String, u64)> {
    let roots: Vec<(String, i32)> = panes
        .iter()
        .filter_map(|p| {
            let info = call(path, "pane.process_info", json!({"pane_id": p})).ok()?;
            Some((p.clone(), info["process_info"]["shell_pid"].as_i64()? as i32))
        })
        .collect();
    if roots.is_empty() {
        return Vec::new();
    }
    let tree = ProcTree::scan(Path::new("/proc"));
    roots.into_iter().map(|(p, pid)| (p, tree.rss(pid))).collect()
}

/// Each non-agent pane that is running something other than a shell.
fn jobs(path: &Path, panes: &[PaneJob], home: &str) -> Vec<PaneJob> {
    panes
        .iter()
        .filter_map(|p| {
            let info = call(path, "pane.process_info", json!({"pane_id": p.pane_id})).ok()?;
            let (command, cwd) = job_from(&info)?;
            let cwd = tilde(if cwd.is_empty() { &p.cwd } else { &cwd }, home);
            Some(PaneJob { command, cwd, ..p.clone() })
        })
        .collect()
}

/// The pass run right after connecting and then every `PERIOD`.
fn pass(path: &Path, panes: &BTreeSet<String>, meta: &[(String, AgentMeta)], job_panes: &[PaneJob], home: &str) -> Vec<Update> {
    let mut out = Vec::new();
    let rows = rss_rows(path, panes);
    if !rows.is_empty() {
        out.push(Update::AgentRss(rows));
    }
    out.push(Update::AgentMeta(meta.to_vec()));
    out.push(Update::Jobs(jobs(path, job_panes, home)));
    out
}

enum End {
    Lost(String),
    ReceiverGone,
}

/// The link state last reported to the UI, so Linked/Lost are sent on changes only.
#[derive(PartialEq)]
enum Reported {
    Nothing,
    Linked,
    Lost,
}

/// One connected session: snapshot, subscribe, then follow events until the link breaks.
fn session(path: &Path, tx: &Sender<Update>, reported: &mut Reported, attempt: &mut usize) -> End {
    macro_rules! send {
        ($u:expr) => {
            if tx.send($u).is_err() {
                return End::ReceiverGone;
            }
        };
    }
    macro_rules! try_link {
        ($r:expr) => {
            match $r {
                Ok(v) => v,
                Err(e) => return End::Lost(e),
            }
        };
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let Snap { agents, workspaces, mut meta, mut job_panes } = try_link!(snapshot(path, &home));
    *attempt = 0;
    if *reported != Reported::Linked {
        send!(Update::HerdrLink(HerdrLink::Linked));
        *reported = Reported::Linked;
    }
    let mut panes = pane_set(&agents);
    send!(Update::HerdrSnapshot { agents, workspaces });
    let mut events = try_link!(subscribe(path, &panes));
    for u in pass(path, &panes, &meta, &job_panes, &home) {
        send!(u);
    }

    let mut last_pass = Instant::now();
    let mut topology_at: Option<Instant> = None;
    // Kept across read timeouts: a timed-out read_line leaves the partial line in the buffer.
    let mut line = String::new();
    loop {
        let periodic_due = last_pass + PERIOD;
        let due = topology_at.map_or(periodic_due, |t| periodic_due.min(t + TOPOLOGY_SETTLE));
        let wait = due.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
        events.get_ref().set_read_timeout(Some(wait)).ok();
        match events.read_line(&mut line) {
            Ok(0) => return End::Lost("herdr event stream closed".into()),
            Ok(_) if line.ends_with('\n') => {
                match parse_event(&line) {
                    Some(Event::Status { pane_id, status, title }) => send!(Update::AgentStatus { pane_id, status, title }),
                    Some(Event::Topology) => {
                        topology_at.get_or_insert_with(Instant::now);
                    }
                    None => {}
                }
                line.clear();
            }
            Ok(_) => {} // partial line right before EOF; the next read returns 0
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return End::Lost(format!("herdr events: {e}")),
        }

        let now = Instant::now();
        if now < due {
            continue;
        }
        let snap = try_link!(snapshot(path, &home));
        (meta, job_panes) = (snap.meta, snap.job_panes);
        let fresh = pane_set(&snap.agents);
        send!(Update::HerdrSnapshot { agents: snap.agents, workspaces: snap.workspaces });
        if fresh != panes {
            events = try_link!(subscribe(path, &fresh));
            panes = fresh;
            line.clear();
        }
        topology_at = None;
        if now >= periodic_due {
            for u in pass(path, &panes, &meta, &job_panes, &home) {
                send!(u);
            }
            last_pass = now;
        }
    }
}

/// Connect, follow, and on any socket failure report `Lost` (once per outage) and retry with
/// `backoff` (the last step repeats). Returns when the receiver is dropped.
pub fn run_loop(path: &Path, tx: Sender<Update>, backoff: &[Duration]) {
    let mut reported = Reported::Nothing;
    let mut attempt = 0;
    loop {
        match session(path, &tx, &mut reported, &mut attempt) {
            End::ReceiverGone => return,
            End::Lost(reason) => {
                if reported != Reported::Lost {
                    if tx.send(Update::HerdrLink(HerdrLink::Lost { since: SystemTime::now(), reason })).is_err() {
                        return;
                    }
                    reported = Reported::Lost;
                }
            }
        }
        let wait = backoff.get(attempt).or(backoff.last()).copied().unwrap_or(BACKOFF[2]);
        attempt += 1;
        thread::sleep(wait);
    }
}

/// `$HERDR_SOCKET_PATH`, else `~/.config/herdr/herdr.sock`.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("HERDR_SOCKET_PATH") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config/herdr/herdr.sock")
}

pub fn spawn(tx: Sender<Update>) {
    thread::spawn(move || run_loop(&socket_path(), tx, &BACKOFF));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc::{self, Receiver};
    use std::thread;
    use std::time::{Duration, SystemTime};

    use crate::state::{HerdrLink, Update};

    fn load(name: &str) -> Value {
        let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        v["result"].clone()
    }

    #[test]
    fn agents_merge_names_and_labels() {
        let (agents, workspaces) = agents_from(&load("herdr_snapshot.json"), &load("herdr_agent_list.json"), SystemTime::UNIX_EPOCH);
        assert_eq!(agents.len(), 3);
        assert_eq!(workspaces.len(), 2);
        let alpha = agents.iter().find(|a| a.name.as_deref() == Some("alpha")).unwrap();
        assert_eq!(alpha.status, AgentStatus::Working);
        assert_eq!(alpha.workspace_label, "search");
        assert!(agents.iter().any(|a| a.name.is_none() && a.status == AgentStatus::Blocked));
    }

    #[test]
    fn parses_status_event_and_ignores_ack() {
        let e = parse_event(r#"{"event":"pane.agent_status_changed","data":{"pane_id":"w1:p2","workspace_id":"w1","agent_status":"blocked","title":"Needs approval"}}"#);
        assert_eq!(e, Some(Event::Status { pane_id: "w1:p2".into(), status: AgentStatus::Blocked, title: Some("Needs approval".into()) }));
        assert_eq!(parse_event(r#"{"id":"sub","result":{"type":"subscription_started"}}"#), None);
        assert_eq!(parse_event(r#"{"event":"pane.closed","data":{"pane_id":"w1:p2"}}"#), Some(Event::Topology));
        assert_eq!(parse_event("not json"), None);
    }

    #[test]
    fn parses_live_wire_spellings() {
        // herdr 0.9.3 sends topology kinds underscored (captured live), status kinds dotted.
        let closed = r#"{"data":{"pane_id":"w58:p3","type":"pane_closed","workspace_id":"w58"},"event":"pane_closed"}"#;
        assert_eq!(parse_event(closed), Some(Event::Topology));
        assert_eq!(parse_event(r#"{"event":"pane_focused","data":{"pane_id":"w1:p1"}}"#), None);
        let no_title = r#"{"event":"pane.agent_status_changed","data":{"pane_id":"w1:p1","workspace_id":"w1","agent_status":"done","title":null}}"#;
        assert_eq!(parse_event(no_title), Some(Event::Status { pane_id: "w1:p1".into(), status: AgentStatus::Done, title: None }));
    }

    #[test]
    fn tree_rss_sums_children() {
        let d = tempfile::tempdir().unwrap();
        // pid 10 (shell) → 11 (claude) → 12 (node helper); 20 unrelated
        for (pid, ppid, rss_kb) in [(10, 1, 4000), (11, 10, 300000), (12, 11, 50000), (20, 1, 999)] {
            let dir = d.path().join(pid.to_string());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("stat"), format!("{pid} (x y) S {ppid} 0 0 0")).unwrap();
            std::fs::write(dir.join("status"), format!("Name:\tx\nVmRSS:\t{rss_kb} kB\n")).unwrap();
        }
        assert_eq!(tree_rss(d.path(), 10), (4000 + 300000 + 50000) * 1024);
        assert_eq!(tree_rss(d.path(), 99), 0);
    }

    #[test]
    fn stat_ppid_takes_field_after_last_paren() {
        assert_eq!(stat_ppid("42 (we (i) rd) S 7 0 0"), Some(7));
        assert_eq!(stat_ppid("garbage"), None);
    }

    #[test]
    fn git_branch_follows_repo_and_worktree_heads() {
        let d = tempfile::tempdir().unwrap();
        let put = |p: &str, v: &str| {
            let path = d.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, v).unwrap();
        };
        put("main/.git/HEAD", "ref: refs/heads/feat/nerv\n");
        put("main/src/deep/x.rs", "");
        put("main/.git/worktrees/wt/HEAD", "ref: refs/heads/wt-branch\n");
        put("wt/.git", &format!("gitdir: {}\n", d.path().join("main/.git/worktrees/wt").display()));
        put("rel/.git", "gitdir: ../main/.git/worktrees/wt\n");
        put("detached/.git/HEAD", "3f1c2a9d0e8b7c6a5f4e3d2c1b0a998877665544\n");
        put("plain/file", "");
        std::fs::create_dir_all(d.path().join("main/stale/.git")).unwrap(); // empty leftover: git skips it
        put("main/badfile/.git", "not a gitdir line\n");
        assert_eq!(git_branch(&d.path().join("main/src/deep")).as_deref(), Some("feat/nerv"));
        assert_eq!(git_branch(&d.path().join("main/stale")).as_deref(), Some("feat/nerv"));
        assert_eq!(git_branch(&d.path().join("main/badfile")).as_deref(), Some("feat/nerv"));
        assert_eq!(git_branch(&d.path().join("wt")).as_deref(), Some("wt-branch"));
        assert_eq!(git_branch(&d.path().join("rel")).as_deref(), Some("wt-branch"));
        assert_eq!(git_branch(&d.path().join("detached")), None);
        assert_eq!(git_branch(&d.path().join("plain")), None);
        assert_eq!(git_branch(&d.path().join("does/not/exist")), None);
    }

    #[test]
    fn job_from_skips_shells_and_basenames_argv0() {
        let shell = serde_json::json!({"process_info": {"shell_pid": 5, "foreground_processes": [
            {"pid": 5, "name": "bash", "argv": ["-bash"], "cwd": "/home/u"}]}});
        assert_eq!(job_from(&shell), None);
        let who = serde_json::json!({"process_info": {"shell_pid": 5, "foreground_processes": [
            {"pid": 5, "name": "zsh", "argv": ["zsh"], "cwd": "/home/u"},
            {"pid": 9, "name": "python3", "argv": ["/usr/bin/python3", "/x/herdr-who"], "cwd": "/home/u/x"}]}});
        assert_eq!(job_from(&who), Some(("python3 /x/herdr-who".to_string(), "/home/u/x".to_string())));
        assert_eq!(job_from(&serde_json::json!({})), None);
    }

    #[test]
    fn tilde_shortens_home_only_at_a_boundary() {
        assert_eq!(tilde("/home/user/Desktop/x", "/home/user"), "~/Desktop/x");
        assert_eq!(tilde("/home/user", "/home/user"), "~");
        assert_eq!(tilde("/home/userx/a", "/home/user"), "/home/userx/a");
        assert_eq!(tilde("/etc", ""), "/etc");
    }

    #[test]
    fn meta_and_job_panes_from_fixtures() {
        let list = load("herdr_agent_list.json");
        let alpha = list["agents"].as_array().unwrap().iter().find(|a| a["name"] == "alpha").unwrap();
        let m = meta_from(alpha, "/home/user");
        assert_eq!(m.cwd, "~/project");
        assert_eq!(m.session_id.as_deref(), Some("00000000-0000-0000-0000-000000000001"));
        assert_eq!(m.branch, None); // fixture cwd does not exist here

        let agents: BTreeSet<String> = ["w1:p1", "w1:p2", "w2:p1"].iter().map(|s| s.to_string()).collect();
        let jobs = job_panes(&load("herdr_snapshot.json"), &agents);
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].pane_id.as_str(), jobs[0].label.as_str()), ("w2:p3", "devserver"));
        assert_eq!((jobs[0].workspace_label.as_str(), jobs[0].tab_label.as_str()), ("ops", "gamma"));
    }

    #[test]
    fn subscriptions_cover_each_agent_pane_and_topology() {
        let panes: BTreeSet<String> = ["w1:p1".to_string(), "w2:p1".to_string()].into();
        let v = subscriptions(&panes);
        let subs = v["subscriptions"].as_array().unwrap();
        let status: Vec<&Value> = subs.iter().filter(|s| s["type"] == "pane.agent_status_changed").collect();
        assert_eq!(status.len(), 2);
        assert!(status.iter().any(|s| s["pane_id"] == "w2:p1"));
        assert!(subs.iter().any(|s| s["type"] == "pane.closed" && s.get("pane_id").is_none()));
    }

    // ---- fake herdr server -------------------------------------------------------------------

    enum Reply {
        /// Write one line, then close.
        Line(String),
        /// Close without answering.
        Close,
        /// Write these lines and keep the connection open (an event stream).
        Hold(Vec<String>),
    }

    const API_ERR: &str = r#"{"id":"x","error":{"code":"pane_not_found","message":"no such pane"}}"#;
    const ACK: &str = r#"{"id":"nerv-events","result":{"type":"subscription_started"}}"#;

    fn fixture_line(name: &str) -> String {
        let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        v.to_string()
    }

    /// Serves one request per connection with `handler(method)`; every request is logged.
    fn fake_herdr(mut handler: impl FnMut(&str, &Value) -> Reply + Send + 'static) -> (tempfile::TempDir, std::path::PathBuf, Receiver<Value>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("herdr.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let (log_tx, log_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut held = Vec::new();
            for conn in listener.incoming() {
                let Ok(mut conn) = conn else { return };
                let mut line = String::new();
                if BufReader::new(&conn).read_line(&mut line).unwrap_or(0) == 0 {
                    continue;
                }
                let req: Value = serde_json::from_str(&line).unwrap();
                let method = req["method"].as_str().unwrap_or("").to_string();
                let reply = handler(&method, &req);
                let _ = log_tx.send(req);
                match reply {
                    Reply::Line(l) => {
                        let _ = writeln!(conn, "{l}");
                    }
                    Reply::Close => {}
                    Reply::Hold(lines) => {
                        for l in lines {
                            let _ = writeln!(conn, "{l}");
                        }
                        held.push(conn);
                    }
                }
            }
        });
        (dir, path, log_rx)
    }

    /// The next `n` link / snapshot / status updates, as short labels.
    fn labels(rx: &Receiver<Update>, n: usize) -> Vec<String> {
        let mut out = Vec::new();
        while out.len() < n {
            match rx.recv_timeout(Duration::from_secs(5)).expect("timed out waiting for updates") {
                Update::HerdrLink(HerdrLink::Linked) => out.push("linked".into()),
                Update::HerdrLink(HerdrLink::Lost { .. }) => out.push("lost".into()),
                Update::HerdrLink(HerdrLink::Connecting) => out.push("connecting".into()),
                Update::HerdrSnapshot { agents, .. } => out.push(format!("snapshot:{}", agents.len())),
                Update::AgentStatus { pane_id, status, .. } => out.push(format!("status:{pane_id}:{}", status.label())),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn reconnects_after_server_drops_connection() {
        let (_dir, path, _log) = fake_herdr(|method, _| match method {
            "session.snapshot" => Reply::Line(fixture_line("herdr_snapshot.json")),
            "agent.list" => Reply::Line(fixture_line("herdr_agent_list.json")),
            "events.subscribe" => Reply::Close,
            _ => Reply::Line(API_ERR.into()),
        });
        let (tx, rx) = mpsc::channel();
        let h = thread::spawn(move || run_loop(&path, tx, &[Duration::from_millis(50)]));
        assert_eq!(labels(&rx, 5), ["linked", "snapshot:3", "lost", "linked", "snapshot:3"]);
        drop(rx);
        h.join().unwrap(); // run_loop returns once the receiver is gone
    }

    #[test]
    fn connect_pass_sends_agent_meta_and_pane_jobs() {
        let (_dir, path, _log) = fake_herdr(|method, req| match method {
            "session.snapshot" => Reply::Line(fixture_line("herdr_snapshot.json")),
            "agent.list" => Reply::Line(fixture_line("herdr_agent_list.json")),
            "events.subscribe" => Reply::Hold(vec![ACK.to_string()]),
            "pane.process_info" if req["params"]["pane_id"] == "w2:p3" => Reply::Line(
                serde_json::json!({"id": "x", "result": {"type": "pane_process_info", "process_info": {
                    "pane_id": "w2:p3", "shell_pid": 1, "foreground_processes": [
                        {"pid": 7, "name": "python3", "argv": ["/usr/bin/python3", "/x/herdr-who"], "cwd": "/home/user/web"}]}}})
                .to_string(),
            ),
            _ => Reply::Line(API_ERR.into()),
        });
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_loop(&path, tx, &[Duration::from_millis(50)]));
        let (mut meta, mut jobs) = (None, None);
        while meta.is_none() || jobs.is_none() {
            match rx.recv_timeout(Duration::from_secs(5)).expect("expected AgentMeta and Jobs") {
                Update::AgentMeta(rows) => meta = Some(rows),
                Update::Jobs(j) => jobs = Some(j),
                _ => {}
            }
        }
        let meta = meta.unwrap();
        assert_eq!(meta.len(), 3);
        assert!(meta.iter().any(|(p, m)| p == "w1:p1" && m.session_id.is_some()));
        let jobs = jobs.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!((jobs[0].label.as_str(), jobs[0].command.as_str()), ("devserver", "python3 /x/herdr-who"));
    }

    #[test]
    fn missing_socket_reports_lost_once_while_retrying() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.sock");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_loop(&path, tx, &[Duration::from_millis(10)]));
        assert_eq!(labels(&rx, 1), ["lost"]);
        thread::sleep(Duration::from_millis(200)); // ~20 retries
        assert!(rx.try_iter().next().is_none(), "Lost must not repeat on every retry");
    }

    #[test]
    fn topology_event_resnapshots_and_resubscribes_new_agent() {
        let mut lists = 0;
        let mut subs = 0;
        let (_dir, path, log) = fake_herdr(move |method, _| match method {
            "session.snapshot" => Reply::Line(fixture_line("herdr_snapshot.json")),
            "agent.list" => {
                lists += 1;
                let mut v: Value = serde_json::from_str(&fixture_line("herdr_agent_list.json")).unwrap();
                if lists > 1 {
                    let agents = v["result"]["agents"].as_array_mut().unwrap();
                    let mut delta = agents[2].clone();
                    delta["pane_id"] = "w2:p2".into();
                    delta["name"] = "delta".into();
                    delta["agent_status"] = "working".into();
                    agents.push(delta);
                }
                Reply::Line(v.to_string())
            }
            "events.subscribe" => {
                subs += 1;
                let mut lines = vec![ACK.to_string()];
                if subs == 1 {
                    lines.push(r#"{"event":"pane.agent_status_changed","data":{"pane_id":"w1:p2","workspace_id":"w1","agent_status":"working"}}"#.into());
                    lines.push(r#"{"data":{"pane":{"pane_id":"w2:p2"},"type":"pane_created"},"event":"pane_created"}"#.into());
                }
                Reply::Hold(lines)
            }
            _ => Reply::Line(API_ERR.into()),
        });
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_loop(&path, tx, &[Duration::from_millis(50)]));
        assert_eq!(labels(&rx, 4), ["linked", "snapshot:3", "status:w1:p2:ACTIVE", "snapshot:4"]);
        let mut sub_reqs = Vec::new();
        while sub_reqs.len() < 2 {
            let req = log.recv_timeout(Duration::from_secs(5)).expect("expected a second events.subscribe");
            if req["method"] == "events.subscribe" {
                sub_reqs.push(req);
            }
        }
        let subs = sub_reqs[1]["params"]["subscriptions"].as_array().unwrap();
        assert!(subs.iter().any(|s| s["type"] == "pane.agent_status_changed" && s["pane_id"] == "w2:p2"));
    }
}
