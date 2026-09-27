use crate::app::{App, Focus, SearchMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handle one key event. Returns true if state changed enough to rerender.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Search prompt active: capture input first.
    if app.search_mode.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.search_mode = None;
                app.search_input.clear();
                if let Some(m) = None::<SearchMode> {
                    let _ = m;
                }
                app.global_hits.clear();
                app.doc_hits.clear();
            }
            KeyCode::Enter => match app.search_mode {
                Some(SearchMode::Global) => {
                    app.run_global_search();
                }
                Some(SearchMode::Document) => {
                    app.run_doc_search();
                    app.search_mode = None;
                }
                _ => {
                    app.search_mode = None;
                }
            },
            KeyCode::Backspace => {
                app.search_input.pop();
                match app.search_mode {
                    Some(SearchMode::TreeFilter) => {
                        if let Some(first) = app.filter_tree().first() {
                            app.tree_cursor = *first;
                        }
                    }
                    Some(SearchMode::Global) => app.run_global_search(),
                    _ => {}
                }
            }
            KeyCode::Char(c) => {
                app.search_input.push(c);
                match app.search_mode {
                    Some(SearchMode::TreeFilter) => {
                        // Live filter; jump cursor to first match.
                        if let Some(first) = app.filter_tree().first() {
                            app.tree_cursor = *first;
                        }
                    }
                    Some(SearchMode::Global) => app.run_global_search(),
                    _ => {}
                }
            }
            _ => {}
        }
        return;
    }

    // Document search results active: n/N navigation.
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
            _ => {}
        }
    }

    // Global hits list open: j/k navigates hits, Enter opens.
    if !app.global_hits.is_empty() && app.search_mode.is_none() {
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
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.doc_hits.is_empty() && app.global_hits.is_empty() {
                app.quit = true;
            } else {
                app.doc_hits.clear();
                app.global_hits.clear();
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit = true;
        }

        // Source switching.
        KeyCode::Tab | KeyCode::Char('s') => {
            app.active_source = match app.active_source {
                crate::docs::Source::Enma => crate::docs::Source::Perception,
                crate::docs::Source::Perception => crate::docs::Source::Enma,
            };
        }

        // Focus switch.
        KeyCode::Char('w') => {
            app.focus = match app.focus {
                Focus::Tree => Focus::Content,
                Focus::Content => Focus::Tree,
            };
        }

        // Search prompts.
        KeyCode::Char('/') => {
            app.search_mode = Some(SearchMode::TreeFilter);
            app.search_input.clear();
            app.focus = Focus::Tree;
        }
        KeyCode::Char('f') => {
            app.search_mode = Some(SearchMode::Global);
            app.search_input.clear();
            app.global_hits.clear();
        }
        KeyCode::Char('*') => {
            app.search_mode = Some(SearchMode::Document);
            app.search_input.clear();
            app.doc_hits.clear();
        }

        // Tree navigation (always available; nvimtree-like).
        KeyCode::Char('j') | KeyCode::Down if app.focus == Focus::Tree => {
            app.tree_move(1)
        }
        KeyCode::Char('k') | KeyCode::Up if app.focus == Focus::Tree => {
            app.tree_move(-1)
        }
        KeyCode::Char('g') => {
            if app.focus == Focus::Tree {
                app.tree_cursor = 0;
            } else {
                app.scroll = 0;
            }
        }
        KeyCode::Char('G') => {
            if app.focus == Focus::Tree {
                app.tree_cursor = app.visible.len().saturating_sub(1);
            } else {
                app.scroll = u16::MAX; // clamped at render
            }
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right if app.focus == Focus::Tree => {
            app.open_cursor_page();
        }
        KeyCode::Char('h') | KeyCode::Left if app.focus == Focus::Tree => {
            app.toggle_expand_at_cursor();
        }

        // Content scrolling (works regardless of focus, vim-ish).
        KeyCode::Char('j') | KeyCode::Down => app.scroll_by(1),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_by(-1),
        KeyCode::Char('d') => app.scroll_by(20),
        KeyCode::Char('u') => app.scroll_by(-20),
        _ => {}
    }
}
