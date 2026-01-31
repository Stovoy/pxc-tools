use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Project {
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

pub fn load_project(_path: &Path) -> Result<Project> {
    bail!("project loader not implemented yet")
}
