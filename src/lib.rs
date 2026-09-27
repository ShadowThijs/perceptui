mod app;
mod docs;
mod events;
mod fetcher;
mod markdown;
mod ui;

use anyhow::Result;
use tokio::sync::mpsc;

pub fn run() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let mut terminal = ratatui::init();
    let result = rt.block_on(async_main(&mut terminal));
    ratatui::restore();
    result
}

/// Background thread pumping crossterm events into a tokio channel.
fn spawn_event_pump() -> mpsc::Receiver<crossterm::event::Event> {
    let (tx, rx) = mpsc::channel(64);
    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(ev) => {
                    if tx.blocking_send(ev).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    rx
}

async fn async_main(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = app::App::new()?;
    let mut input_rx = spawn_event_pump();

    loop {
        // Drain sync progress messages.
        while let Ok(msg) = app.sync_rx.try_recv() {
            apply_sync_msg(&mut app, msg);
        }

        terminal.draw(|f| ui::render(f, &mut app))?;

        if app.quit {
            return Ok(());
        }

        tokio::select! {
            ev = input_rx.recv() => {
                match ev {
                    Some(crossterm::event::Event::Key(key)) => {
                        if key.kind == crossterm::event::KeyEventKind::Press {
                            events::handle_key(&mut app, key);
                        }
                    }
                    Some(_) => {}
                    None => return Ok(()),
                }
            }
            msg = app.sync_rx.recv() => {
                if let Some(msg) = msg {
                    apply_sync_msg(&mut app, msg);
                }
            }
        }
    }
}

fn apply_sync_msg(app: &mut app::App, msg: fetcher::SyncMsg) {
    match msg {
        fetcher::SyncMsg::Starting { total } => {
            app.sync_status = format!("downloading 0/{total}");
        }
        fetcher::SyncMsg::Progress { done, total } => {
            app.sync_status = format!("{done}/{total}");
        }
        fetcher::SyncMsg::Done { updated } => {
            app.syncing = false;
            app.sync_status = format!("{updated} pages updated");
            app.reload_index_if_needed();
            app.rerender();
        }
        fetcher::SyncMsg::Failed(e) => {
            app.syncing = false;
            app.sync_status = format!("sync failed: {e}");
        }
    }
}
