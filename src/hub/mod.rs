//! `nerv hub`: ratatui view for a herdr split pane. Redraws at 4 Hz; `q`, `Esc` or `Ctrl-C` quit.

pub mod widgets;

use std::time::{Duration, SystemTime};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::cli::HubArgs;
use crate::collect::Which;
use crate::config::Config;
use crate::state::{drain_into, State};

const FRAME: Duration = Duration::from_millis(250);

pub fn run(args: HubArgs) -> anyhow::Result<()> {
    let config = Config::load()?;
    // The hub skips cava: the spectrum is TV-only.
    let rx = crate::start_feed(args.demo, Which { spectrum: false }, &config);
    let mut state = State::new(SystemTime::now());
    // `ratatui::init` enters raw mode + the alternate screen and installs a panic hook that
    // restores the terminal before the panic message prints.
    let mut terminal = ratatui::init();
    let result = (|| -> anyhow::Result<()> {
        loop {
            drain_into(&rx, &mut state);
            state.tick(SystemTime::now());
            terminal.draw(|f| widgets::render(f, &state, &config))?;
            if event::poll(FRAME)? {
                if let Event::Key(k) = event::read()? {
                    let ctrl_c = k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL);
                    if k.kind == KeyEventKind::Press && (ctrl_c || matches!(k.code, KeyCode::Char('q') | KeyCode::Esc))
                    {
                        return Ok(());
                    }
                }
            }
        }
    })();
    ratatui::restore();
    result
}
