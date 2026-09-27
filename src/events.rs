use crate::app::{App, Focus, SearchMode};
use crate::docs::Source;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handle one key event.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Global shortcuts (work everywhere).
    if ctrl {
        match key.code {
            KeyCode::Char('c') => app.quit = true,
            KeyCode::Char('n') => app.tree_visible = !app.tree_visible,
            KeyCode::Char('h') => app.focus = Focus::Tree,
            KeyCode::Char('l') => app.focus = Focus::Content,
            _ => {}
        }
        return;
    }

    // Search prompt active.
    if app.search_mode.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.search_mode = None;
                app.search_input.clear();
            }
            KeyCode::Enter => {
                let mode = app.search_mode.take().unwrap();
                match mode {
                    SearchMode::Document => {
                        app.run_doc_search();
                        if app.doc_hits.is_empty() {
                            app.doc_query.clear();
                        }
                    }
                    SearchMode::Global => {
                        app.run_global_search();
                        if app.global_hits.is_empty() {
                            app.search_mode = None;
                        }
                    }
                    SearchMode::TreeFilter => {
                        app.open_cursor_page();
                        app.search_input.clear();
                    }
                }
            }
            KeyCode::Backspace => {
                app.search_input.pop();
                on_search_changed(app);
            }
            KeyCode::Char(c) => {
                app.search_input.push(c);
                on_search_changed(app);
            }
            _ => {}
        }
        return;
    }

    // Document search results active: n/N navigation, Esc clears.
    if !app.doc_hits.is_empty() {
        match key.code {
            KeyCode::Char('n') => {
                app.next_doc_hit(1);
                return;
            }
            KeyCode::Char('N') => {
                app.next_doc_hit(-1);
                return;
            }
            KeyCode::Esc => {
                app.doc_hits.clear();
                app.doc_query.clear();
                return;
            }
            _ => {}
        }
    }

    // Global hits popup: j/k navigates, Enter opens, Esc closes.
    if !app.global_hits.is_empty() {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                app.global_cursor =
                    (app.global_cursor + 1).min(app.global_hits.len() - 1);
                return;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.global_cursor = app.global_cursor.saturating_sub(1);
                return;
            }
            KeyCode::Enter => {
                let hit = &app.global_hits[app.global_cursor];
                let page = hit.page.clone();
                app.active_source = hit.source;
                app.open(page);
                app.global_hits.clear();
                app.focus = Focus::Content;
                return;
            }
            KeyCode::Esc => {
                app.global_hits.clear();
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => app.quit = true,

        // Source switching (both trees always visible).
        KeyCode::Tab => {
            app.active_source = match app.active_source {
                Source::Enma => Source::Perception,
                Source::Perception => Source::Enma,
            };
        }

        // Search: / targets the focused pane (tree filter or in-page).
        KeyCode::Char('/') => {
            app.search_mode = Some(if app.focus == Focus::Tree {
                SearchMode::TreeFilter
            } else {
                SearchMode::Document
            });
            app.search_input.clear();
        }
        // Global search across all docs.
        KeyCode::Char('f') => {
            app.search_mode = Some(SearchMode::Global);
            app.search_input.clear();
            app.global_hits.clear();
        }

        // Tree navigation.
        KeyCode::Char('j') | KeyCode::Down if app.focus == Focus::Tree => {
            app.tree_move(1)
        }
        KeyCode::Char('k') | KeyCode::Up if app.focus == Focus::Tree => {
            app.tree_move(-1)
        }
        KeyCode::Char('g') if app.focus == Focus::Tree => app.tree_cursor = 0,
        KeyCode::Char('G') if app.focus == Focus::Tree => {
            app.tree_cursor = app.visible.len().saturating_sub(1);
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right
            if app.focus == Focus::Tree =>
        {
            app.open_cursor_page();
        }
        KeyCode::Char('h') | KeyCode::Left if app.focus == Focus::Tree => {
            app.toggle_expand_at_cursor();
        }

        // Content scrolling.
        KeyCode::Char('j') | KeyCode::Down => app.scroll_by(1),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_by(-1),
        KeyCode::Char('d') => app.scroll_by(20),
        KeyCode::Char('u') => app.scroll_by(-20),
        KeyCode::Char('g') => app.scroll = 0,
        KeyCode::Char('G') => app.scroll = u16::MAX,
        _ => {}
    }
}

fn on_search_changed(app: &mut App) {
    match app.search_mode {
        Some(SearchMode::TreeFilter) => {
            // Jump to first match and live-open it.
            if let Some(first) = app.filter_tree().first() {
                app.tree_cursor = *first;
                app.preview_cursor_page();
            }
        }
        Some(SearchMode::Global) => app.run_global_search(),
        _ => {}
    }
}
