mod executor;

pub use executor::Runtime;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    clipboard::Sensitivity,
    visual::{Notification, OverlayRequest},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Type(String),
    Paste {
        text: String,
        sensitivity: Sensitivity,
        overlay_reset: Option<String>,
    },
    Info {
        file: Option<PathBuf>,
        path: Vec<String>,
        type_text: bool,
        sensitivity: Sensitivity,
    },
    Key(String),
    Combo(Vec<String>),
    Sleep(u64),
    Notify(Notification),
    Overlay(OverlayRequest),
}
