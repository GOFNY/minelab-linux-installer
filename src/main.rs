mod download;
mod manifest;
mod progress;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use futures_util::{StreamExt, stream};
use reqwest::Client;

use crate::download::{download_mod, needs_download};
use crate::manifest::{ModEntry, fetch_manifest};
use crate::progress::Progress;

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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mods_dir = args.game_dir.join("mods");

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
    let pending = find_pending(&mods_dir, manifest.mods).await?;

    if pending.is_empty() {
        println!("Todos os {mod_count} mods já estão instalados");
        return Ok(());
    }

    let total_bytes: u64 = pending.iter().map(|entry| entry.tamanho).sum();

    let progress = Progress::new(total_bytes);
    let client = &client;
    let mods_dir = &mods_dir;
    let progress = &progress;

    let results: Vec<std::result::Result<(), String>> = stream::iter(pending)
        .map(move |entry| async move {
            let name = entry.arquivo.clone();
            download_mod(client, mods_dir, &entry, progress)
                .await
                .map_err(|error| format!("{name}: {error:#}"))
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let failures: Vec<String> = results
        .into_iter()
        .filter_map(|result| result.err())
        .collect();
    progress.finish(failures.is_empty());

    if !failures.is_empty() {
        eprintln!("{} mod(s) falharam:", failures.len());
        for failure in &failures {
            eprintln!("  {failure}");
        }
        bail!("installation failed");
    }

    println!("{mod_count} mods instalados");
    Ok(())
}
