use crate::app::{App, Focus, GlobalHit, SearchMode};
use crate::docs::Source;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line as RLine, Span as RSpan};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn render(f: &mut Frame, app: &mut App) {
    // Status line at the bottom, content above.
    let [body, _status] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    if app.tree_visible {
        let [tree, content] = Layout::horizontal([
            Constraint::Percentage(20),
            Constraint::Percentage(80),
        ])
        .areas(body);

        app.content_width = content.width.saturating_sub(4).max(20);
        if app.last_render_width != app.content_width {
            app.last_render_width = app.content_width;
            app.rerender();
        }

        render_tree(f, app, tree);
        render_content(f, app, content);
    } else {
        app.content_width = body.width.saturating_sub(4).max(20);
        if app.last_render_width != app.content_width {
            app.last_render_width = app.content_width;
            app.rerender();
        }
        render_content(f, app, body);
    }
    render_status(f, app, f.area());

    // Overlays.
    if app.search_mode.is_some() {
        render_search_prompt(f, app);
    }
    if !app.global_hits.is_empty() && app.search_mode.is_none() {
        render_global_hits(f, app, f.area());
    }
}

fn render_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Tree;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused { ACCENT } else { DIM }))
        .title(RSpan::styled(
            format!(" {} ", app.active_source.title()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(RLine::from(RSpan::styled(
            " [/ filter]",
            Style::default().fg(DIM),
        )).alignment(ratatui::layout::Alignment::Center));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let filter = app.filter_tree();
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .enumerate()
        .filter(|(i, _)| filter.contains(i))
        .map(|(i, node)| {
            let indent = "  ".repeat(node.depth);
            let is_cursor = focused && i == app.tree_cursor;
            let (icon, style) = match (&node.page, node.depth) {
                (None, 0) => ("◈ ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                (None, _) => ("▸ ", Style::default().fg(Color::Blue)),
                (Some(_), _) => ("  ", Style::default()),
            };
            let mut spans = vec![
                RSpan::styled(indent, Style::default()),
                RSpan::styled(icon, style),
            ];
            if is_cursor {
                spans.push(RSpan::styled(
                    node.title.clone(),
                    Style::default().fg(Color::Black).bg(ACCENT),
                ));
            } else {
                spans.push(RSpan::styled(node.title.clone(), style));
            }
            ListItem::new(RLine::from(spans))
        })
        .collect();

    let mut state = ListState::default();
    // Map absolute cursor index into the filtered list position.
    let pos = filter.iter().position(|&i| i == app.tree_cursor);
    if let Some(p) = pos {
        state.select(Some(p));
    }
    let list = List::new(items).highlight_symbol(">");
    f.render_stateful_widget(list, inner, &mut state);
}

fn render_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Content;
    let title = app
        .open_page
        .as_ref()
        .map(|p| p.title.clone())
        .unwrap_or_else(|| "no page".into());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused { ACCENT } else { DIM }))
        .title(RSpan::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    let max_scroll = app.rendered.len().saturating_sub(height.saturating_sub(1)) as u16;
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }
    let start = app.scroll as usize;

    let lines: Vec<RLine> = app
        .rendered
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(i, line)| {
            let mut spans: Vec<RSpan> = line
                .spans
                .iter()
                .map(|s| RSpan::styled(s.content.clone(), s.style))
                .collect();
            // Highlight in-page search: dim for hits, bright bg for current.
            if !app.doc_query.is_empty() {
                let q = app.doc_query.to_lowercase();
                let is_hit = app.doc_hits.contains(&i);
                let is_current = is_hit
                    && app.doc_hits.get(app.doc_hit_cursor) == Some(&i);
                if is_current {
                    spans = highlight_word(&spans, &q, Color::Rgb(120, 100, 0));
                } else if is_hit {
                    spans = highlight_word(&spans, &q, Color::Rgb(60, 55, 20));
                }
            }
            RLine::from(spans)
        })
        .collect();

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);

    // Scrollbar.
    if app.rendered.len() > height {
        let track = inner.height as usize;
        let frac = start as f64 / (app.rendered.len() - height).max(1) as f64;
        let thumb_h = (track as f64 * height as f64 / app.rendered.len() as f64).max(1.0) as usize;
        let thumb_pos = ((track - thumb_h) as f64 * frac) as usize;
        for y in 0..track {
            let cell = f.buffer_mut().cell_mut(ratatui::layout::Position::new(inner.x + inner.width, inner.y + y as u16));
            if let Some(c) = cell {
                let on = y >= thumb_pos && y < thumb_pos + thumb_h;
                c.set_symbol("▐");
                c.set_style(Style::default().fg(if on { ACCENT } else { DIM }));
            }
        }
    }
}

/// Split spans on case-insensitive matches of `q`, giving them a bg color.
fn highlight_word<'a>(spans: &[RSpan<'a>], q: &str, bg: Color) -> Vec<RSpan<'a>> {
    let mut out: Vec<RSpan> = Vec::new();
    for span in spans {
        let mut rest: &str = &span.content;
        let lower = rest.to_lowercase();
        let mut consumed = 0usize;
        while let Some(pos) = lower[consumed..].find(q) {
            let start = consumed + pos;
            let end = start + q.len();
            if start > consumed {
                out.push(RSpan::styled(
                    rest[consumed..start].to_string(),
                    span.style,
                ));
            }
            out.push(RSpan::styled(
                rest[start..end].to_string(),
                span.style.bg(bg),
            ));
            consumed = end;
        }
        if consumed < rest.len() {
            out.push(RSpan::styled(rest[consumed..].to_string(), span.style));
        }
        let _ = rest;
    }
    out
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let sync = if app.syncing {
        app.sync_status.clone()
    } else {
        format!("docs synced ({})", app.sync_status)
    };
    let page_info = app
        .open_page
        .as_ref()
        .map(|p| p.url.clone())
        .unwrap_or_default();
    let hits = if !app.doc_hits.is_empty() {
        format!("  [{}/{} doc matches, n/N]", app.doc_hit_cursor + 1, app.doc_hits.len())
    } else {
        String::new()
    };
    let line = RLine::from(vec![
        RSpan::styled(" [Tab] source ", Style::default().fg(DIM)),
        RSpan::styled(" [/] search ", Style::default().fg(DIM)),
        RSpan::styled(" [f] find ", Style::default().fg(DIM)),
        RSpan::styled(" [^n] tree ", Style::default().fg(DIM)),
        RSpan::styled(" [^h/l] focus ", Style::default().fg(DIM)),
        RSpan::styled(" [q] quit ", Style::default().fg(DIM)),
        RSpan::styled(format!("│ {sync} │ {page_info}{hits}"), Style::default().fg(DIM)),
    ]);
    let bar = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    f.render_widget(Paragraph::new(line), bar);
}

fn render_search_prompt(f: &mut Frame, app: &App) {
    let label = match app.search_mode {
        Some(SearchMode::TreeFilter) => "filter",
        Some(SearchMode::Global) => "search docs",
        Some(SearchMode::Document) => "search page",
        None => "",
    };
    let area = Rect::new(0, f.area().height.saturating_sub(2), f.area().width, 1);
    let line = RLine::from(vec![
        RSpan::styled(format!("/{label}: "), Style::default().fg(ACCENT)),
        RSpan::styled(
            format!("{}█", app.search_input),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_global_hits(f: &mut Frame, app: &mut App, area: Rect) {
    let w = area.width * 70 / 100;
    let h = area.height * 70 / 100;
    let x = (area.width - w) / 2;
    let y = (area.height - h) / 2;
    let popup = Rect::new(x, y, w, h);
    f.render_widget(ratatui::widgets::Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .title(RSpan::styled(
            format!(" docs search: {} ", app.search_input),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(RSpan::styled(" [j/k] move  [Enter] open  [Esc] close ", Style::default().fg(DIM)));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let items: Vec<ListItem> = app
        .global_hits
        .iter()
        .enumerate()
        .map(|(i, GlobalHit { source, page, excerpt })| {
            let tag = match source {
                Source::Enma => "ENMA",
                Source::Perception => "PERC",
            };
            let selected = i == app.global_cursor;
            let style = if selected {
                Style::default().fg(Color::Black).bg(ACCENT)
            } else {
                Style::default()
            };
            let tag_style = if selected {
                style
            } else {
                match source {
                    Source::Enma => Style::default().fg(Color::Green),
                    Source::Perception => Style::default().fg(Color::LightRed),
                }
            };
            ListItem::new(RLine::from(vec![
                RSpan::styled(format!(" [{tag}] "), tag_style),
                RSpan::styled(page.title.clone(), style.add_modifier(Modifier::BOLD)),
                RSpan::styled(format!("  {excerpt}"), style),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.global_cursor));
    let list = List::new(items).highlight_symbol(">");
    f.render_stateful_widget(list, inner, &mut state);
}
