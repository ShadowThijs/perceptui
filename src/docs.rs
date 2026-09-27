use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One documentation source (site).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Enma,
    Perception,
}

impl Source {
    pub const ALL: [Source; 2] = [Source::Enma, Source::Perception];

    pub fn title(self) -> &'static str {
        match self {
            Source::Enma => "Enma",
            Source::Perception => "Perception",
        }
    }

    /// llms.txt manifest URL listing all markdown pages.
    pub fn manifest_url(self) -> &'static str {
        match self {
            Source::Enma => "https://enma.studio/docs/enma/llms.txt",
            Source::Perception => "https://docs.perception.cx/perception/llms.txt",
        }
    }

    /// Directory inside the docs root holding this source's pages.
    pub fn dir_name(self) -> &'static str {
        match self {
            Source::Enma => "enma",
            Source::Perception => "perception",
        }
    }
}

/// A single documentation page parsed out of llms.txt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageEntry {
    /// Page title from the llms.txt link text.
    pub title: String,
    /// Absolute URL of the raw markdown file.
    pub url: String,
}

impl PageEntry {
    /// Local file name derived from the URL path.
    pub fn file_name(&self) -> String {
        let path = self.url.split("://").nth(1).unwrap_or(&self.url);
        let path = path.split('/').collect::<Vec<_>>().join("__");
        format!("{path}.md")
    }
}

pub fn docs_root() -> PathBuf {
    // /tmp/enma-docs on Linux, %TEMP%\enma-docs on Windows.
    std::env::temp_dir().join("enma-docs")
}

pub fn source_dir(source: Source) -> PathBuf {
    docs_root().join(source.dir_name())
}

pub fn index_path() -> PathBuf {
    docs_root().join("index.json")
}

/// Full index: every page of every source.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub pages: Vec<(SourceKind, PageEntry)>,
}

/// Serde-friendly source tag (enums with data are annoying in JSON).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Enma,
    Perception,
}

impl From<Source> for SourceKind {
    fn from(s: Source) -> Self {
        match s {
            Source::Enma => SourceKind::Enma,
            Source::Perception => SourceKind::Perception,
        }
    }
}

impl From<SourceKind> for Source {
    fn from(k: SourceKind) -> Self {
        match k {
            SourceKind::Enma => Source::Enma,
            SourceKind::Perception => Source::Perception,
        }
    }
}

/// Parse llms.txt into page entries: markdown links to .md files.
pub fn parse_manifest(source: Source, llms: &str) -> Vec<PageEntry> {
    let mut pages = Vec::new();
    for line in llms.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("- [") else { continue };
        let Some((title, rest)) = rest.split_once("](") else { continue };
        let Some((url, _desc)) = rest.split_once(')') else { continue };
        if !url.ends_with(".md") || !url.starts_with("http") {
            continue;
        }
        // Skip the llms manifest itself.
        if url.ends_with("llms.txt") {
            continue;
        }
        // Outdated subsections of a source are never fetched.
        if is_excluded(source, &url) {
            continue;
        }
        pages.push(PageEntry {
            title: title.trim().to_string(),
            url: url.to_string(),
        });
    }
    // Dedup by url, keep first.
    let mut seen = std::collections::HashSet::new();
    pages.retain(|p| seen.insert(p.url.clone()));
    pages
}

/// Perception subsections that contain outdated information; never fetched.
const PERCEPTION_EXCLUDED: &[&str] = &[
    "perception/angel-script/",
    "perception/lua-script/",
    "perception/docs/",
    "perception/perception-ide/",
];

fn is_excluded(source: Source, url: &str) -> bool {
    match source {
        Source::Enma => false,
        Source::Perception => PERCEPTION_EXCLUDED.iter().any(|p| url.contains(p)),
    }
}

pub fn load_index() -> Result<Index> {
    let data = std::fs::read_to_string(index_path()).context("no docs index on disk")?;
    serde_json::from_str(&data).context("corrupt docs index")
}

pub fn save_index(index: &Index) -> Result<()> {
    std::fs::create_dir_all(docs_root())?;
    std::fs::write(index_path(), serde_json::to_string_pretty(index)?)?;
    Ok(())
}

pub fn page_path(entry: &PageEntry) -> PathBuf {
    source_dir(source_of(&entry.url)).join(entry.file_name())
}

fn source_of(url: &str) -> Source {
    if url.contains("enma.studio") {
        Source::Enma
    } else {
        Source::Perception
    }
}
