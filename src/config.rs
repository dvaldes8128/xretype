use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub info: InfoConfig,
    pub input: InputConfig,
    pub feedback: FeedbackConfig,
    pub automation: AutomationConfig,
    pub xremap: XremapConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InfoConfig {
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutomationConfig {
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct XremapConfig {
    pub root: Option<PathBuf>,
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputConfig {
    pub backend: InputBackendKind,
    pub paste_delay_ms: u64,
    pub clipboard_serve_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputBackendKind {
    #[default]
    Libei,
    Ydotool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            backend: InputBackendKind::Libei,
            paste_delay_ms: 30,
            clipboard_serve_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeedbackConfig {
    pub notify_errors: bool,
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            notify_errors: true,
        }
    }
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let path = explicit_path
            .map(Path::to_path_buf)
            .or_else(default_config_path);
        let Some(path) = path else {
            return Ok(Self::default());
        };

        if explicit_path.is_none() && !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("could not read config file {}", path.display()))?;
        toml::from_str(&contents)
            .with_context(|| format!("could not parse config file {}", path.display()))
    }

    pub fn info_file(&self) -> PathBuf {
        self.info.file.clone().unwrap_or_else(default_info_path)
    }

    pub fn automation_file(&self) -> PathBuf {
        self.automation
            .file
            .clone()
            .unwrap_or_else(crate::automation::default_automation_path)
    }

    pub fn xremap_root(&self) -> PathBuf {
        self.xremap.root.clone().unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from(".config"))
                .join("xremap")
        })
    }
}

fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|directory| directory.join("xretype/config.toml"))
}

fn default_info_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("personal_info.json")
}

#[cfg(test)]
mod tests {
    use super::{Config, InputBackendKind};

    #[test]
    fn config_defaults_are_safe_for_short_lived_clipboard_use() {
        let config = Config::default();
        assert_eq!(config.input.backend, InputBackendKind::Libei);
        assert_eq!(config.input.paste_delay_ms, 30);
        assert!(config.input.clipboard_serve_ms >= config.input.paste_delay_ms);
        assert!(config.feedback.notify_errors);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let config: Config = toml::from_str("[input]\npaste_delay_ms = 50").unwrap();
        assert_eq!(config.input.paste_delay_ms, 50);
        assert_eq!(config.input.clipboard_serve_ms, 500);
    }

    #[test]
    fn selects_ydotool_backend() {
        let config: Config = toml::from_str("[input]\nbackend = \"ydotool\"").unwrap();
        assert_eq!(config.input.backend, InputBackendKind::Ydotool);
    }
}
