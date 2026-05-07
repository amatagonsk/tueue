mod app;
mod ui;

use std::{env, io::stdout};

use color_eyre::Result;
use crossterm::{event::EnableMouseCapture, ExecutableCommand};

use app::App;

fn main() -> Result<()> {
    color_eyre::install()?;
    let args: Vec<_> = env::args().skip(1).collect();
    if args.first().map_or(false, |a| a == "-h" || a == "--help") {
        eprintln!("Usage: tueue [pueue status args...]");
        eprintln!();
        eprintln!("A TUI monitor for `pueue status`.");
        eprintln!();
        eprintln!("Arguments after the command name are passed directly to `pueue status`.");
        eprintln!("Example: tueue -- -g mygroup");
        return Ok(());
    }
    stdout().execute(EnableMouseCapture)?;
    let terminal = ratatui::init();
    let app_result = App::new(args.join(" ")).run(terminal);
    ratatui::restore();
    app_result
}
