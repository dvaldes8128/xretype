pub mod json;
mod literal;
mod stdin;

use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug, Clone)]
pub enum ValueSource {
    Literal(String),
    Stdin,
    Json { file: PathBuf, path: Vec<String> },
}

impl ValueSource {
    pub fn resolve(self) -> Result<String> {
        match self {
            Self::Literal(value) => Ok(literal::resolve(value)),
            Self::Stdin => stdin::resolve(),
            Self::Json { file, path } => json::resolve(&file, &path),
        }
    }
}
