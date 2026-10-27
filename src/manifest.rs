use std::time::Duration;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ModEntry {
    pub arquivo: String,
    pub url: String,
    pub sha1: String,
    pub tamanho: u64,
}

#[derive(Deserialize)]
pub struct Manifest {
    pub mods: Vec<ModEntry>,
}

pub async fn fetch_manifest(client: &Client, url: &str) -> Result<Manifest> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}
