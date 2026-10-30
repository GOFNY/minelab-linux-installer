mod download;
mod manifest;
mod progress;
mod state;

use std::collections::BTreeSet;
use std::io::ErrorKind;

use crate::download::{download_mod, needs_download};
use crate::progress::Progress;
use crate::state::State;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use futures_util::{StreamExt, stream};
use reqwest::Client;

use crate::manifest::{ModEntry, fetch_manifest};

const MANIFEST_URL: &str = "https://kauedev.com/minelab/mods.json";
const CONCURRENCY: usize = 4;

#[derive(Parser)]
struct Args {
    game_dir: PathBuf,
}

fn confirm(message: &str) -> Result<bool> {
    print!("{message} (s/N) ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("s"))
}

async fn remove_stale(mods_dir: &Path, previous: &State, current: &BTreeSet<String>) -> Result<()> {
    for name in previous.mods.difference(current) {
        match tokio::fs::remove_file(mods_dir.join(name)).await {
            Ok(()) => println!("{name}: removido"),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("{name}: could not remove the file"));
            }
        }
    }
    Ok(())
}

async fn find_pending(mods_dir: &Path, mods: Vec<ModEntry>) -> Result<Vec<ModEntry>> {
    let bar = progress::verification_bar(mods.len() as u64);

    let checked: Vec<Result<Option<ModEntry>>> = stream::iter(mods)
        .map(|entry| {
            let bar = &bar;
            async move {
                let pending = needs_download(mods_dir, &entry)
                    .await
                    .with_context(|| format!("{}: could not verify the file", entry.arquivo))?;
                bar.inc(1);
                if !pending {
                    bar.println(format!("{}: já instalado", entry.arquivo));
                }
                Ok::<_, anyhow::Error>(pending.then_some(entry))
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    bar.finish_and_clear();
    checked.into_iter().filter_map(Result::transpose).collect()
}

async fn download_pending(
    client: &Client,
    mods_dir: &Path,
    pending: Vec<ModEntry>,
) -> Vec<(String, String)> {
    let total_bytes: u64 = pending.iter().map(|entry| entry.tamanho).sum();
    let progress = Progress::new(total_bytes);
    let progress = &progress;

    let results: Vec<Result<(), (String, String)>> = stream::iter(pending)
        .map(move |entry| async move {
            download_mod(client, mods_dir, &entry, progress)
                .await
                .map_err(|error| (entry.arquivo.clone(), format!("{error:#}")))
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let failures: Vec<(String, String)> = results.into_iter().filter_map(Result::err).collect();
    progress.finish(failures.is_empty());
    failures
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mods_dir = args.game_dir.join("mods");
    let state_path = args.game_dir.join("minelab.json");

    println!("Mods serão instalados em: {}", mods_dir.display());
    if !confirm("Continuar?")? {
        return Ok(());
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()?;

    let manifest = fetch_manifest(&client, MANIFEST_URL).await?;
    let mod_count = manifest.mods.len();
    let current: BTreeSet<String> = manifest
        .mods
        .iter()
        .map(|entry| entry.arquivo.clone())
        .collect();

    let previous = state::load(&state_path).await?;
    remove_stale(&mods_dir, &previous, &current).await?;

    let pending = find_pending(&mods_dir, manifest.mods).await?;
    let failures = if pending.is_empty() {
        Vec::new()
    } else {
        download_pending(&client, &mods_dir, pending).await
    };

    let mut installed = current;
    for (name, _) in &failures {
        installed.remove(name);
    }
    state::save(&state_path, &State { mods: installed }).await?;

    if !failures.is_empty() {
        eprintln!("{} mod(s) falharam:", failures.len());
        for (name, error) in &failures {
            eprintln!("  {name}: {error}");
        }
        bail!("installation failed");
    }

    println!("{mod_count} mods instalados");
    Ok(())
}
