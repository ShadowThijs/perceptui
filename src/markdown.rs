use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

/// A styled run of text on one logical line.
#[derive(Clone, Debug)]
pub struct Span {
    pub content: String,
    pub style: Style,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    fn plain(content: impl Into<String>) -> Self {
        Line {
            spans: vec![Span {
                content: content.into(),
                style: Style::default(),
            }],
        }
    }

    fn styled(content: impl Into<String>, style: Style) -> Self {
        Line {
            spans: vec![Span {
                content: content.into(),
                style,
            }],
        }
    }

    fn width(&self) -> usize {
        self.spans.iter().map(|s| s.content.width()).sum()
    }
}

const H1: Style = Style::new()
    .fg(Color::Cyan)
    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
const H2: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
const H3: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
const H4: Style = Style::new().fg(Color::Blue);
const CODE_BLOCK: Style = Style::new().fg(Color::Green);
const INLINE_CODE: Style = Style::new().fg(Color::Magenta);
const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);
const ITALIC: Style = Style::new().add_modifier(Modifier::ITALIC);
const STRIKE: Style = Style::new().add_modifier(Modifier::CROSSED_OUT);
const LINK: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED);
const QUOTE: Style = Style::new().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
const RULE: Style = Style::new().fg(Color::DarkGray);
const LIST_MARKER: Style = Style::new().fg(Color::LightBlue);
const TABLE_BORDER: Style = Style::new().fg(Color::DarkGray);
const TABLE_HEAD: Style = Style::new().add_modifier(Modifier::BOLD);

/// Parse markdown into styled lines. Wraps to `width` columns when width > 0.
pub fn render(md: &str, width: usize) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut lines = md.lines().peekable();
    let mut in_code = false;
    let mut code_lang: Option<String> = None;
    let mut code_buf: Vec<String> = Vec::new();
    let mut list_depth = 0usize;

    while let Some(raw) = lines.next() {
        // Fenced code blocks.
        let trimmed = raw.trim_start();
        if let Some(fence) = trimmed.strip_prefix("```") {
            if in_code {
                out.extend(render_code_block(&code_lang.take().unwrap_or_default(), &code_buf, width));
                code_buf.clear();
                in_code = false;
            } else {
                in_code = true;
                code_lang = Some(fence.trim().to_string());
            }
            continue;
        }
        if in_code {
            code_buf.push(raw.to_string());
            continue;
        }

        if raw.trim().is_empty() {
            out.push(Line::plain(""));
            continue;
        }

        // Headings.
        if let Some(h) = trimmed.strip_prefix("# ") {
            out.push(blank());
            out.push(Line::styled(strip_inline(h), H1));
            out.push(Line::styled("─".repeat(width.min(120)), RULE));
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("## ") {
            out.push(blank());
            out.push(Line::styled(strip_inline(h), H2));
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("### ") {
            out.push(blank());
            out.push(Line::styled(strip_inline(h), H3));
            continue;
        }
        for level in 4..=6 {
            let prefix = format!("{} ", "#".repeat(level));
            if let Some(h) = trimmed.strip_prefix(&prefix) {
                out.push(blank());
                out.push(Line::styled(strip_inline(h), H4));
                break;
            }
        }
        if trimmed.starts_with('#') {
            continue;
        }

        // Horizontal rule.
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push(blank());
            out.push(Line::styled("─".repeat(width.min(120)), RULE));
            continue;
        }

        // Blockquote.
        if let Some(q) = trimmed.strip_prefix('>') {
            let inner = q.trim().trim_start_matches('>').trim();
            for l in wrap_lines(&render_inline(inner), width.saturating_sub(2).max(20)) {
                let mut spans = vec![Span {
                    content: "▌ ".to_string(),
                    style: Style::new().fg(Color::DarkGray),
                }];
                spans.extend(l.spans);
                out.push(Line { spans });
            }
            continue;
        }

        // Table.
        if trimmed.starts_with('|') && is_table_row(trimmed) {
            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut first = true;
            let mut header_cells = Vec::new();
            // Gather the full table.
            let mut table_lines = vec![trimmed.to_string()];
            while let Some(next) = lines.peek() {
                let n = next.trim_start();
                if n.starts_with('|') && is_table_row(n) {
                    table_lines.push(n.to_string());
                    lines.next();
                } else {
                    break;
                }
            }
            for (i, tl) in table_lines.iter().enumerate() {
                if is_separator_row(tl) {
                    continue;
                }
                let cells = parse_table_row(tl);
                if first {
                    header_cells = cells.clone();
                    first = false;
                }
                let _ = i;
                rows.push(cells);
            }
            out.extend(render_table(&header_cells, &rows, width));
            continue;
        }

        // Lists (nested by indentation).
        let indent = raw.len() - raw.trim_start().len();
        let new_depth = indent / 2;
        if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            list_depth = new_depth;
            let marker = format!("{}• ", "  ".repeat(list_depth));
            let avail = width.saturating_sub(marker.width()).max(20);
            let rendered = wrap_lines(&render_inline(rest), avail);
            for (i, l) in rendered.into_iter().enumerate() {
                if i == 0 {
                    let mut spans = vec![Span {
                        content: marker.clone(),
                        style: LIST_MARKER,
                    }];
                    spans.extend(l.spans);
                    out.push(Line { spans });
                } else {
                    let pad = " ".repeat(marker.width());
                    let mut spans = vec![Span::default_content(pad)];
                    spans.extend(l.spans);
                    out.push(Line { spans });
                }
            }
            continue;
        }
        if let Some(rest) = ordered_marker(trimmed) {
            list_depth = new_depth;
            let marker = format!("{}{}. ", "  ".repeat(list_depth), rest.0);
            let avail = width.saturating_sub(marker.width()).max(20);
            let rendered = wrap_lines(&render_inline(rest.1), avail);
            for (i, l) in rendered.into_iter().enumerate() {
                if i == 0 {
                    let mut spans = vec![Span {
                        content: marker,
                        style: LIST_MARKER,
                    }];
                    spans.extend(l.spans);
                    out.push(Line { spans });
                } else {
                    let mut spans = vec![Span::default_content(" ".repeat(
                        marker.len(),
                    ))];
                    spans.extend(l.spans);
                    out.push(Line { spans });
                }
            }
            continue;
        }
        let _ = list_depth;

        // Paragraph text: gather consecutive plain lines into one paragraph.
        let mut para = vec![strip_inline_raw(trimmed)];
        while let Some(next) = lines.peek() {
            let n = next.trim_start();
            if n.is_empty()
                || n.starts_with('#')
                || n.starts_with("```")
                || n.starts_with('>')
                || n.starts_with("| ")
                || n.starts_with("- ")
                || n.starts_with("* ")
                || n == "---"
            {
                break;
            }
            para.push(strip_inline_raw(n));
            lines.next();
        }
        let joined = para.join(" ");
        out.extend(wrap_lines(&render_inline(&joined), width.max(20)));
    }

    // Unterminated code block.
    if in_code {
        out.extend(render_code_block(&code_lang.unwrap_or_default(), &code_buf, width));
    }

    // Trim leading/trailing blanks.
    while out.first().map(|l| l.spans.is_empty() || l.width() == 0) == Some(true) {
        out.remove(0);
    }
    while out.last().map(|l| l.width() == 0) == Some(true) {
        out.pop();
    }
    out
}

fn blank() -> Line {
    Line::plain("")
}

fn ordered_marker(line: &str) -> Option<(String, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i + 1 >= line.len() || !line[i..].starts_with(". ") {
        return None;
    }
    Some((line[..i].to_string(), &line[i + 2..]))
}

fn strip_inline(s: &str) -> String {
    render_inline(s)
        .spans
        .into_iter()
        .map(|s| s.content)
        .collect()
}

fn strip_inline_raw(s: &str) -> String {
    strip_inline(s)
}

/// Inline markdown -> single styled Line (no wrapping).
fn render_inline(s: &str) -> Line {
    let mut spans: Vec<Span> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut buf = String::new();
    let mut i = 0;

    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                spans.push(Span {
                    content: std::mem::take(&mut buf),
                    style: Style::default(),
                });
            }
        };
    }

    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        // Images: skip to alt text -> render as link-ish.
        if rest.starts_with("![") {
            if let Some(end) = find_close(&chars, i + 1, '[', ']') {
                let alt: String = chars[i + 2..end].iter().collect();
                flush!();
                spans.push(Span {
                    content: format!("[image: {alt}]"),
                    style: Style::new().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                });
                // Skip the (url) part.
                let mut j = end + 1;
                if j < chars.len() && chars[j] == '(' {
                    if let Some(close) = find_close(&chars, j, '(', ')') {
                        j = close + 1;
                    }
                }
                i = j;
                continue;
            }
        }
        // Links [text](url).
        if rest.starts_with('[') {
            if let Some(end) = find_close(&chars, i, '[', ']') {
                let after: String = chars[end + 1..].iter().collect();
                if after.starts_with('(') {
                    if let Some(close) = find_close(&chars, end + 1, '(', ')') {
                        let text: String = chars[i + 1..end].iter().collect();
                        let url: String = chars[end + 2..close].iter().collect();
                        flush!();
                        let label = if url.starts_with('#') || url.is_empty() {
                            text
                        } else {
                            format!("{text} ↗")
                        };
                        spans.push(Span {
                            content: label,
                            style: LINK,
                        });
                        i = close + 1;
                        continue;
                    }
                }
            }
        }
        // Bold+italic.
        if rest.starts_with("***") {
            if let Some(end) = rest[3..].find("***") {
                flush!();
                spans.push(Span {
                    content: rest[3..3 + end].to_string(),
                    style: BOLD.patch(ITALIC),
                });
                i += 3 + end + 3;
                continue;
            }
        }
        // Bold.
        if rest.starts_with("**") {
            if let Some(end) = rest[2..].find("**") {
                flush!();
                spans.push(Span {
                    content: rest[2..2 + end].to_string(),
                    style: BOLD,
                });
                i += 2 + end + 2;
                continue;
            }
        }
        // Strikethrough.
        if rest.starts_with("~~") {
            if let Some(end) = rest[2..].find("~~") {
                flush!();
                spans.push(Span {
                    content: rest[2..2 + end].to_string(),
                    style: STRIKE,
                });
                i += 2 + end + 2;
                continue;
            }
        }
        // Italic (single * or _).
        if rest.starts_with('*') || (rest.starts_with('_') && !rest.starts_with("__")) {
            let (delim, dlen): (char, usize) = if rest.starts_with('*') { ('*', 1) } else { ('_', 1) };
            if let Some(end) = rest[dlen..].find(delim) {
                let inner = &rest[dlen..dlen + end];
                if !inner.is_empty() {
                    flush!();
                    spans.push(Span {
                        content: inner.to_string(),
                        style: ITALIC,
                    });
                    i += dlen + end + dlen;
                    continue;
                }
            }
        }
        // Inline code.
        if rest.starts_with('`') {
            if let Some(end) = rest[1..].find('`') {
                flush!();
                spans.push(Span {
                    content: rest[1..1 + end].to_string(),
                    style: INLINE_CODE,
                });
                i += 1 + end + 1;
                continue;
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    if !buf.is_empty() {
        spans.push(Span {
            content: buf,
            style: Style::default(),
        });
    }
    Line { spans }
}

fn find_close(chars: &[char], from: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (i, &c) in chars.iter().enumerate().skip(from) {
        if c == open {
            depth += 1;
        } else if c == close {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
    }
    None
}

fn render_code_block(lang: &str, code: &[String], width: usize) -> Vec<Line> {
    let mut out = Vec::new();
    let inner = width.saturating_sub(4).max(20);
    let border = Style::new().fg(Color::DarkGray);
    let lang_disp = if lang.is_empty() { "text" } else { lang };
    out.push(Line {
        spans: vec![
            Span {
                content: format!("╭─ {lang_disp} ", ),
                style: border,
            },
            Span {
                content: "─".repeat(inner.saturating_sub(lang_disp.len() + 4)),
                style: border,
            },
            Span {
                content: "╮".to_string(),
                style: border,
            },
        ],
    });
    for line in code {
        let truncated: String = line.chars().take(inner).collect();
        out.push(Line {
            spans: vec![
                Span {
                    content: "│ ".to_string(),
                    style: border,
                },
                Span {
                    content: truncated,
                    style: CODE_BLOCK,
                },
                Span {
                    content: " │".to_string(),
                    style: border,
                },
            ],
        });
    }
    out.push(Line {
        spans: vec![
            Span {
                content: "╰".to_string(),
                style: border,
            },
            Span {
                content: "─".repeat(inner + 2),
                style: border,
            },
            Span {
                content: "╯".to_string(),
                style: border,
            },
        ],
    });
    out.push(Line::plain(""));
    out
}

fn is_table_row(line: &str) -> bool {
    line.matches('|').count() >= 2
}

fn is_separator_row(line: &str) -> bool {
    let compact: String = line.chars().filter(|&c| c != '|' && c != ' ').collect();
    !compact.is_empty() && compact.chars().all(|c| c == '-' || c == ':')
}

fn parse_table_row(line: &str) -> Vec<String> {
    let s = line.trim().trim_start_matches('|').trim_end_matches('|');
    s.split('|')
        .map(|c| strip_inline(c.trim()))
        .collect()
}

fn render_table(header: &[String], rows: &[Vec<String>], width: usize) -> Vec<Line> {
    let n_cols = header.len().max(1);
    let inner = width.saturating_sub(n_cols * 3 + 1).max(10);
    let mut col_widths = vec![0usize; n_cols];
    let flat = rows.iter().flat_map(|r| r.iter());
    for (i, cell) in flat.enumerate() {
        let c = i % n_cols;
        col_widths[c] = col_widths[c].max(cell.width().min(inner));
    }
    let total: usize = col_widths.iter().sum::<usize>() + n_cols * 3 + 1;
    let _ = total;
    let mut out = Vec::new();

    let sep = |l: char, m: char, r: char| -> Line {
        let mut content = String::from(l);
        for (i, w) in col_widths.iter().enumerate() {
            content.push_str(&"─".repeat(w + 2));
            content.push(if i + 1 == n_cols { r } else { m });
        }
        Line::styled(content, TABLE_BORDER)
    };

    out.push(sep('┌', '┬', '┐'));
    let mut push_row = |cells: &[String], style: Style| {
        let mut spans = vec![Span {
            content: "│ ".to_string(),
            style: TABLE_BORDER,
        }];
        for (i, w) in col_widths.iter().enumerate() {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let pad = w.saturating_sub(cell.width());
            spans.push(Span {
                content: format!("{cell}{}", " ".repeat(pad)),
                style,
            });
            spans.push(Span {
                content: if i + 1 == n_cols { " │" } else { " │ " }.to_string(),
                style: TABLE_BORDER,
            });
        }
        out.push(Line { spans });
    };
    push_row(header, TABLE_HEAD);
    out.push(sep('├', '┼', '┤'));
    for row in rows {
        push_row(row, Style::default());
    }
    out.push(sep('└', '┴', '┘'));
    out.push(Line::plain(""));
    out
}

/// Word-wrap styled spans at `width`. Returns new lines of spans.
fn wrap_lines(line: &Line, width: usize) -> Vec<Line> {
    if width == 0 {
        return vec![line.clone()];
    }
    let mut result: Vec<Line> = Vec::new();
    let mut cur: Vec<Span> = Vec::new();
    let mut cur_w = 0usize;

    let mut push_word = |cur: &mut Vec<Span>, cw: &mut usize, result: &mut Vec<Line>, word: &str, style: Style| {
        let ww = UnicodeWidthStr::width(word);
        if *cw + ww > width && *cw > 0 {
            result.push(Line {
                spans: std::mem::take(cur),
            });
            *cw = 0;
        }
        cur.push(Span {
            content: word.to_string(),
            style,
        });
        *cw += ww;
    };

    for span in &line.spans {
        // Split span into words, keeping spaces attached to preceding word.
        let mut word = String::new();
        let mut is_space = false;
        let words: Vec<(String, bool)> = {
            let mut v = Vec::new();
            for ch in span.content.chars() {
                let sp = ch.is_whitespace();
                if sp == is_space && !word.is_empty() {
                    v.push((std::mem::take(&mut word), is_space));
                }
                is_space = sp;
                word.push(ch);
            }
            if !word.is_empty() {
                v.push((word, is_space));
            }
            v
        };
        for (w, sp) in words {
            if sp {
                // Whitespace: attach unless at line start.
                if cur_w > 0 {
                    push_word(&mut cur, &mut cur_w, &mut result, &w, span.style);
                }
            } else {
                push_word(&mut cur, &mut cur_w, &mut result, &w, span.style);
            }
        }
    }
    if !cur.is_empty() {
        result.push(Line { spans: cur });
    }
    if result.is_empty() {
        result.push(Line::plain(""));
    }
    result
}

impl Span {
    fn default_content(content: String) -> Self {
        Span {
            content,
            style: Style::default(),
        }
    }
}
