use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};
use futures_util::StreamExt;
use indicatif::ProgressBar;
use reqwest::header::RANGE;
use reqwest::{Client, StatusCode};
use sha1::{Digest, Sha1};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::manifest::ModEntry;
use crate::progress::Progress;

const ATTEMPTS: u32 = 3;

pub async fn needs_download(mods_dir: &Path, entry: &ModEntry) -> Result<bool> {
    let dest = mods_dir.join(&entry.arquivo);
    Ok(!matches_sha1(&dest, &entry.sha1).await?)
}

pub async fn download_mod(
    client: &Client,
    mods_dir: &Path,
    entry: &ModEntry,
    progress: &Progress,
) -> Result<()> {
    fs::create_dir_all(mods_dir).await?;
    let dest = mods_dir.join(&entry.arquivo);

    let bar = progress.add_mod(&entry.arquivo, entry.tamanho);
    let result = download_with_retry(client, entry, &dest, &bar, progress.overall()).await;
    bar.finish_and_clear();
    result
}

async fn download_with_retry(
    client: &Client,
    entry: &ModEntry,
    dest: &Path,
    bar: &ProgressBar,
    overall: &ProgressBar,
) -> Result<()> {
    let part = dest.with_file_name(format!("{}.part", entry.arquivo));

    for attempt in 1..ATTEMPTS {
        if try_download(client, entry, dest, &part, bar, overall)
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(u64::from(attempt))).await;
    }
    try_download(client, entry, dest, &part, bar, overall).await
}

async fn try_download(
    client: &Client,
    entry: &ModEntry,
    dest: &Path,
    part: &Path,
    bar: &ProgressBar,
    overall: &ProgressBar,
) -> Result<()> {
    let resume_from = partial_size(part).await;

    let mut request = client.get(entry.url.as_str());
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let response = request.send().await?;

    let status = response.status();
    if status == StatusCode::RANGE_NOT_SATISFIABLE {
        fs::remove_file(part).await?;
        bail!("server rejected the Range header, restarting from zero");
    }
    let response = response.error_for_status()?;

    let resumed = status == StatusCode::PARTIAL_CONTENT;
    sync_position(bar, overall, if resumed { resume_from } else { 0 });

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(resumed)
        .truncate(!resumed)
        .open(part)
        .await?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        bar.inc(chunk.len() as u64);
        overall.inc(chunk.len() as u64);
    }
    file.flush().await?;
    drop(file);

    if !sha1_of(part).await?.eq_ignore_ascii_case(&entry.sha1) {
        fs::remove_file(part).await?;
        bail!("sha1 mismatch");
    }
    fs::rename(part, dest).await?;
    Ok(())
}

fn sync_position(bar: &ProgressBar, overall: &ProgressBar, new_position: u64) {
    let old_position = bar.position();
    bar.set_position(new_position);
    if new_position >= old_position {
        overall.inc(new_position - old_position);
    } else {
        overall.set_position(
            overall
                .position()
                .saturating_sub(old_position - new_position),
        );
    }
}

async fn partial_size(path: &Path) -> u64 {
    fs::metadata(path)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

async fn matches_sha1(path: &Path, expected: &str) -> Result<bool> {
    if !fs::try_exists(path).await? {
        return Ok(false);
    }
    Ok(sha1_of(path).await?.eq_ignore_ascii_case(expected))
}

async fn sha1_of(path: &Path) -> Result<String> {
    let mut file = File::open(path).await?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}
