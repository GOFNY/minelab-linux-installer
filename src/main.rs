mod download;
mod manifest;
mod progress;

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Parser;
use futures_util::{StreamExt, stream};
use reqwest::Client;

use crate::download::install_mod;
use crate::manifest::fetch_manifest;
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
    let total_bytes: u64 = manifest.mods.iter().map(|entry| entry.tamanho).sum();

    let progress = Progress::new(total_bytes);
    let client = &client;
    let mods_dir = &mods_dir;
    let progress = &progress;

    let results: Vec<std::result::Result<(), String>> = stream::iter(manifest.mods)
        .map(move |entry| async move {
            let name = entry.arquivo.clone();
            install_mod(client, mods_dir, &entry, progress)
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
