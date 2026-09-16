use std::io::{self, Read};

use anyhow::{Context, Result};

pub fn resolve() -> Result<String> {
    let mut value = String::new();
    io::stdin()
        .read_to_string(&mut value)
        .context("could not read standard input")?;
    Ok(value.trim_end_matches(['\r', '\n']).to_owned())
}
