use crate::docs::{self, Index, PageEntry, Source};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::sync::mpsc;

/// Messages the background sync worker sends to the UI.
#[derive(Debug, Clone)]
pub enum SyncMsg {
    /// Full download progress: (done, total).
    Starting { total: usize },
    Progress { done: usize, total: usize },
    Done { updated: usize },
    Failed(String),
}

pub async fn fetch(url: &str) -> Result<String> {
    let body = reqwest::get(url).await?.error_for_status()?.text().await?;
    Ok(body)
}

/// Download all pages for a source into the docs dir, returns fetched count.
async fn download_all(
    client: &reqwest::Client,
    source: Source,
    pages: &[PageEntry],
    done: &mut usize,
    total: usize,
    tx: &mpsc::Sender<SyncMsg>,
) -> Result<usize> {
    let dir = docs::source_dir(source);
    tokio::fs::create_dir_all(&dir).await?;
    let mut written = 0usize;
    for page in pages {
        let body = fetch_inner(client, &page.url).await?;
        let path = docs::page_path(page);
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(&path, &body).await?;
        *done += 1;
        let _ = tx
            .send(SyncMsg::Progress {
                done: *done,
                total,
            })
            .await;
        written += 1;
    }
    Ok(written)
}

async fn fetch_inner(client: &reqwest::Client, url: &str) -> Result<String> {
    Ok(client.get(url).send().await?.error_for_status()?.text().await?)
}

fn hash_file(path: &Path) -> Option<[u8; 32]> {
    let data = std::fs::read(path).ok()?;
    Some(Sha256::digest(&data).into())
}

/// Background worker. If docs exist on disk: hash-check each live page and
/// replace only changed files. Otherwise: full download. Never blocks the UI.
pub async fn sync_worker(tx: mpsc::Sender<SyncMsg>) {
    if let Err(e) = sync_inner(&tx).await {
        let _ = tx.send(SyncMsg::Failed(format!("{e:#}"))).await;
    }
}

async fn sync_inner(tx: &mpsc::Sender<SyncMsg>) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("enma-tui/0.1")
        .build()?;
    let have_index = docs::load_index().is_ok();

    // Fetch both manifests first so we know the total.
    let mut all: Vec<(Source, Vec<PageEntry>)> = Vec::new();
    for source in Source::ALL {
        let llms = fetch_inner(&client, source.manifest_url()).await?;
        all.push((source, docs::parse_manifest(source, &llms)));
    }
    let total: usize = all.iter().map(|(_, p)| p.len()).sum();
    let _ = tx.send(SyncMsg::Starting { total }).await;

    let mut index = Index::default();
    let mut done = 0usize;
    let mut updated = 0usize;

    for (source, pages) in all {
        if have_index {
            // Incremental: replace only files whose live hash differs.
            let dir = docs::source_dir(source);
            tokio::fs::create_dir_all(&dir).await?;
            for page in &pages {
                let body = fetch_inner(&client, &page.url).await?;
                let path = docs::page_path(page);
                let live_hash: [u8; 32] = Sha256::digest(body.as_bytes()).into();
                let same = hash_file(&path) == Some(live_hash);
                if !same {
                    tokio::fs::write(&path, &body).await?;
                    updated += 1;
                }
                done += 1;
                let _ = tx
                    .send(SyncMsg::Progress { done, total })
                    .await;
            }
        } else {
            updated += download_all(&client, source, &pages, &mut done, total, tx).await?;
        }
        for page in pages {
            index.pages.push((source.into(), page));
        }
    }

    docs::save_index(&index)?;
    let _ = tx.send(SyncMsg::Done { updated }).await;
    Ok(())
}
