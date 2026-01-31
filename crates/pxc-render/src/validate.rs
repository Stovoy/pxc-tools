use anyhow::{bail, Result};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn validate_project(_path: &Path) -> Result<ValidationReport> {
    bail!("validation pipeline not implemented yet")
}
