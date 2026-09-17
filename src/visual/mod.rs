mod notification;
mod overlay;

pub use notification::notify_error;
pub use overlay::{
    OverlayOperation, OverlayRequest, OverlayView, active_name as active_overlay_name,
    run_host as run_overlay_host,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub timeout_ms: u32,
}

pub trait VisualBackend {
    fn notify(&mut self, notification: &Notification) -> Result<()>;
    fn overlay(&mut self, request: &OverlayRequest) -> Result<()>;
}

pub struct DesktopVisual {
    xremap_root: PathBuf,
}

impl DesktopVisual {
    pub fn new(xremap_root: PathBuf) -> Self {
        Self { xremap_root }
    }
}

impl VisualBackend for DesktopVisual {
    fn notify(&mut self, notification: &Notification) -> Result<()> {
        notification::show(notification)
    }

    fn overlay(&mut self, request: &OverlayRequest) -> Result<()> {
        overlay::handle(&self.xremap_root, request)
    }
}
