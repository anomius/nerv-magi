mod cli;
mod collect;
mod config;
mod control;
mod demo;
mod history;
mod hub;
mod state;
mod tv;

use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use clap::Parser;

use crate::cli::{Cli, Command, DumpArgs};
use crate::config::Config;
use crate::state::{drain_into, State, Update};

/// Start the data feed for a view: synthetic (`demo`) or live collectors.
pub fn start_feed(demo: bool, which: collect::Which, config: &Config) -> mpsc::Receiver<Update> {
    let (tx, rx) = mpsc::channel();
    if demo {
        demo::spawn(tx);
    } else {
        collect::spawn_all(tx, which, config);
    }
    rx
}

fn dump(args: DumpArgs) -> anyhow::Result<()> {
    let config = Config::load()?;
    let rx = start_feed(args.demo, collect::Which { spectrum: true }, &config);
    let mut state = State::new(SystemTime::now());
    let mut next = Instant::now() + Duration::from_millis(1500);
    loop {
        while Instant::now() < next {
            drain_into(&rx, &mut state);
            std::thread::sleep(Duration::from_millis(50));
        }
        state.tick(SystemTime::now());
        println!("{}", serde_json::to_string_pretty(&state)?);
        if args.once {
            return Ok(());
        }
        next = Instant::now() + Duration::from_secs(1);
    }
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Tv(a) => tv::run(a),
        Command::Hub(a) => hub::run(a),
        Command::Dump(a) => dump(a),
    }
}
