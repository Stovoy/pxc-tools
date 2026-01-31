use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RenderConfig {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub frame: u32,
    pub preview_node: Option<String>,
    pub validate_only: bool,
}

#[derive(Clone, Debug)]
pub struct RenderResult {
    pub output_path: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
}

pub fn render_project(_config: RenderConfig) -> Result<RenderResult> {
    bail!("render pipeline not implemented yet")
}
