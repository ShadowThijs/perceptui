mod app;
mod docs;
mod events;
mod fetcher;
mod markdown;
mod ui;

use anyhow::Result;

pub fn run() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = app::App::new()?.run(&mut terminal);
    ratatui::restore();
    result
}
