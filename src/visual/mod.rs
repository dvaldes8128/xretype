mod notification;
mod overlay;

pub use notification::notify_error;
pub use overlay::{OverlayOperation, OverlayRequest};

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub timeout_ms: u32,
}

pub trait VisualBackend {
    fn notify(&mut self, notification: &Notification) -> Result<()>;
    fn overlay(&mut self, request: &OverlayRequest) -> Result<()>;
}

pub struct DesktopVisual;

impl VisualBackend for DesktopVisual {
    fn notify(&mut self, notification: &Notification) -> Result<()> {
        notification::show(notification)
    }

    fn overlay(&mut self, request: &OverlayRequest) -> Result<()> {
        overlay::handle(request)
    }
}
