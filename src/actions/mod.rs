mod executor;

pub use executor::Runtime;

use crate::{
    clipboard::Sensitivity,
    visual::{Notification, OverlayRequest},
};

#[derive(Debug, Clone)]
pub enum Action {
    Type(String),
    Paste {
        text: String,
        sensitivity: Sensitivity,
    },
    Key(String),
    Combo(Vec<String>),
    Sleep(u64),
    Notify(Notification),
    Overlay(OverlayRequest),
}
