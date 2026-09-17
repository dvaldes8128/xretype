mod enigo;
mod ydotool;

pub use enigo::{EnigoInput, parse_key};
pub use ydotool::YdotoolInput;

use anyhow::Result;

pub trait InputBackend {
    fn type_text(&mut self, text: &str) -> Result<()>;
    fn key(&mut self, key: &str) -> Result<()>;
    fn combo(&mut self, keys: &[String]) -> Result<()>;
}
