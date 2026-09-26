use crate::docs::{self, Index, PageEntry, Source};
use crate::fetcher::{self, SyncMsg};
use crate::markdown::{self, Line};
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Content,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Tree filter (nvimtree-style prompt at tree top).
    TreeFilter,
    /// Full-documentation search (all pages, both sources shown tagged).
    Global,
    /// In-document search over the rendered page.
    Document,
}

pub struct App {
    pub index: Option<Index>,
    pub focus: Focus,
    pub active_source: Source,
    pub tree_roots: HashMap<Source, Vec<usize>>,
    pub expanded: std::collections::HashSet<String>,
    pub tree_cursor: usize,
    pub visible: Vec<VisibleNode>,

    pub open_page: Option<PageEntry>,
    pub rendered: Vec<Line>,
    pub scroll: u16,
    pub content_width: u16,

    pub search_mode: Option<SearchMode>,
    pub search_input: String,
    pub global_hits: Vec<GlobalHit>,
    pub global_cursor: usize,
    pub doc_hits: Vec<usize>,
    pub doc_hit_cursor: usize,

    pub sync_rx: mpsc::Receiver<SyncMsg>,
    pub sync_status: String,
    pub syncing: bool,
    pub quit: bool,
}

/// Flat tree node for rendering.
pub struct VisibleNode {
    pub source: Source,
    pub title: String,
    pub page: Option<PageEntry>,
    pub depth: usize,
}

pub struct GlobalHit {
    pub source: Source,
    pub page: PageEntry,
    /// Matching line excerpt.
    pub excerpt: String,
}

impl App {
    pub fn new() -> Result<Self> {
        let index = docs::load_index().ok();
        let (tx, sync_rx) = mpsc::channel(256);
        // Background sync starts immediately; never blocks first render.
        tokio::spawn(fetcher::sync_worker(tx));
        let mut app = Self {
            index,
            focus: Focus::Tree,
            active_source: Source::Enma,
            tree_roots: HashMap::new(),
            expanded: std::collections::HashSet::new(),
            tree_cursor: 0,
            visible: Vec::new(),
            open_page: None,
            rendered: Vec::new(),
            scroll: 0,
            content_width: 80,
            search_mode: None,
            search_input: String::new(),
            global_hits: Vec::new(),
            global_cursor: 0,
            doc_hits: Vec::new(),
            doc_hit_cursor: 0,
            sync_rx,
            sync_status: "syncing…".into(),
            syncing: true,
            quit: false,
        };
        app.rebuild_tree();
        if app.open_page.is_none() {
            app.open_first_page();
        }
        Ok(app)
    }

    /// Load index from disk if we did not have one yet (after first sync).
    pub fn reload_index_if_needed(&mut self) {
        if self.index.is_none() {
            if let Ok(idx) = docs::load_index() {
                self.index = Some(idx);
                self.rebuild_tree();
                self.open_first_page();
            }
        }
    }

    fn open_first_page(&mut self) {
        let Some(idx) = &self.index else { return };
        let entry = idx
            .pages
            .iter()
            .find(|(k, _)| Source::from(*k) == self.active_source)
            .map(|(_, p)| p.clone());
        if let Some(page) = entry {
            self.open(page);
        }
    }

    /// Rebuild flat visible tree from index, grouped by source with folders
    /// derived from URL path segments.
    pub fn rebuild_tree(&mut self) {
        let Some(idx) = self.index.clone() else { return };
        self.visible.clear();
        for source in Source::ALL {
            let pages: Vec<&PageEntry> = idx
                .pages
                .iter()
                .filter(|(k, _)| Source::from(*k) == source)
                .map(|(_, p)| p)
                .collect();
            if pages.is_empty() {
                continue;
            }
            self.visible.push(VisibleNode {
                source,
                title: source.title().to_string(),
                page: None,
                depth: 0,
            });
            // Group by first path segment after the root, when there is one.
            let mut folders: Vec<(String, Vec<&PageEntry>)> = Vec::new();
            for p in pages {
                let seg = folder_of(&p.url);
                match seg {
                    Some(seg) => {
                        let e = folders.iter_mut().find(|(f, _)| *f == seg);
                        match e {
                            Some((_, v)) => v.push(p),
                            None => folders.push((seg, vec![p])),
                        }
                    }
                    None => {
                        folders.push((String::new(), vec![p]));
                    }
                }
            }
            folders.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
            for (folder, fps) in folders {
                if folder.is_empty() {
                    for p in fps {
                        self.visible.push(VisibleNode {
                            source,
                            title: p.title.clone(),
                            page: Some(p.clone()),
                            depth: 1,
                        });
                    }
                } else {
                    let key = format!("{}/{}", source.dir_name(), folder);
                    let open = self.expanded.contains(&key);
                    self.visible.push(VisibleNode {
                        source,
                        title: format!("{folder}{}", if open { " ▾" } else { " ▸" }),
                        page: None,
                        depth: 1,
                    });
                    if open {
                        for p in fps {
                            self.visible.push(VisibleNode {
                                source,
                                title: p.title.clone(),
                                page: Some(p.clone()),
                                depth: 2,
                            });
                        }
                    }
                }
            }
        }
        self.tree_cursor = self.tree_cursor.min(self.visible.len().saturating_sub(1));
    }

    pub fn open(&mut self, page: PageEntry) {
        let path = docs::page_path(&page);
        if let Ok(md) = std::fs::read_to_string(&path) {
            self.rendered = markdown::render(&md, self.content_width as usize);
            self.open_page = Some(page);
            self.scroll = 0;
            self.doc_hits.clear();
            self.doc_hit_cursor = 0;
        }
    }

    pub fn rerender(&mut self) {
        if let Some(page) = self.open_page.clone() {
            let scroll = self.scroll;
            self.open(page);
            self.scroll = scroll;
        }
    }

    pub fn toggle_expand_at_cursor(&mut self) {
        if let Some(node) = self.visible.get(self.tree_cursor) {
            if node.page.is_none() && node.depth > 0 {
                let folder = folder_title(&node.title);
                let key = format!("{}/{}", node.source.dir_name(), folder);
                if !self.expanded.remove(&key) {
                    self.expanded.insert(key);
                }
                self.rebuild_tree();
            }
        }
    }

    pub fn tree_move(&mut self, delta: i32) {
        let len = self.visible.len() as i32;
        if len == 0 {
            return;
        }
        let cur = self.tree_cursor as i32 + delta;
        self.tree_cursor = cur.clamp(0, len - 1) as usize;
    }

    pub fn open_cursor_page(&mut self) {
        let Some(node) = self.visible.get(self.tree_cursor) else {
            return;
        };
        if let Some(page) = node.page.clone() {
            self.active_source = node.source;
            self.open(page);
            self.focus = Focus::Content;
        } else if node.depth > 0 {
            self.toggle_expand_at_cursor();
        }
    }

    /// Filtered visible nodes for the tree prompt.
    pub fn filter_tree(&self) -> Vec<usize> {
        if self.search_mode != Some(SearchMode::TreeFilter) || self.search_input.is_empty() {
            return (0..self.visible.len()).collect();
        }
        let q = self.search_input.to_lowercase();
        self.visible
            .iter()
            .enumerate()
            .filter(|(_, n)| n.title.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    /// Search all cached pages. Runs on the main thread but over local files,
    /// so it stays fast (dozens of small md files).
    pub fn run_global_search(&mut self) {
        let Some(idx) = &self.index else { return };
        let q = self.search_input.to_lowercase();
        self.global_hits.clear();
        if q.is_empty() {
            return;
        }
        for (k, page) in &idx.pages {
            let source = Source::from(*k);
            let path = docs::page_path(page);
            let Ok(md) = std::fs::read_to_string(&path) else {
                continue;
            };
            let mut count = 0usize;
            let mut excerpt = String::new();
            for line in md.lines() {
                if line.to_lowercase().contains(&q) {
                    count += 1;
                    if excerpt.is_empty() {
                        excerpt = truncate_excerpt(line, &q);
                    }
                }
            }
            if count > 0 {
                let _ = count;
                self.global_hits.push(GlobalHit {
                    source,
                    page: page.clone(),
                    excerpt,
                });
            }
        }
        self.global_cursor = 0;
    }

    pub fn run_doc_search(&mut self) {
        let q = self.search_input.to_lowercase();
        self.doc_hits.clear();
        self.doc_hit_cursor = 0;
        if q.is_empty() {
            return;
        }
        // Search rendered lines (plain concatenation of spans).
        for (i, line) in self.rendered.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.content.as_str()).collect();
            if text.to_lowercase().contains(&q) {
                self.doc_hits.push(i);
            }
        }
        if let Some(&first) = self.doc_hits.first() {
            self.scroll_to_line(first);
        }
    }

    pub fn next_doc_hit(&mut self, delta: i32) {
        if self.doc_hits.is_empty() {
            return;
        }
        let len = self.doc_hits.len() as i32;
        let cur = self.doc_hit_cursor as i32;
        self.doc_hit_cursor = ((cur + delta).rem_euclid(len)) as usize;
        self.scroll_to_line(self.doc_hits[self.doc_hit_cursor]);
    }

    fn scroll_to_line(&mut self, line: usize) {
        let height = 30usize; // approx; ui clamps anyway
        self.scroll = line.saturating_sub(height / 3) as u16;
    }

    pub fn scroll_by(&mut self, delta: i32) {
        self.scroll = (self.scroll as i32 + delta).max(0) as u16;
    }
}

fn folder_of(url: &str) -> Option<String> {
    // https://enma.studio/docs/enma/language-guide/basics.md -> "language-guide"
    let after_root = if let Some(r) = url.strip_prefix("https://enma.studio/docs/enma/") {
        r
    } else if let Some(r) = url.strip_prefix("https://docs.perception.cx/perception/") {
        r
    } else {
        return None;
    };
    let segs: Vec<&str> = after_root.trim_end_matches(".md").split('/').collect();
    if segs.len() > 1 {
        Some(segs[segs.len() - 2].to_string())
    } else {
        None
    }
}

fn folder_title(title: &str) -> String {
    title.trim_end_matches('▸').trim_end_matches('▾').trim().to_string()
}

fn truncate_excerpt(line: &str, q: &str) -> String {
    let lower = line.to_lowercase();
    let Some(pos) = lower.find(&q.to_lowercase()) else {
        return line.chars().take(80).collect();
    };
    let start = lower
        .floor_char_boundary(pos.saturating_sub(30))
        .min(pos);
    let end = lower.ceil_char_boundary((pos + q.len() + 50).min(line.len()));
    let mut s = line[start..end].trim().to_string();
    if start > 0 {
        s = format!("…{s}");
    }
    if end < line.len() {
        s.push('…');
    }
    s
}
