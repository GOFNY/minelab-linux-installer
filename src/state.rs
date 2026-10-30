use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub mods: BTreeSet<String>,
}

pub async fn load(path: &Path) -> Result<State> {
    match fs::read_to_string(path).await {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(State::default()),
        Err(error) => Err(error.into()),
    }
}

pub async fn save(path: &Path, state: &State) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(state)?).await?;
    Ok(())
}
