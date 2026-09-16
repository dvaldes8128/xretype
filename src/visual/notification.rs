use anyhow::{Context, Result};
use notify_rust::{Notification as DesktopNotification, Timeout};

use super::Notification;

pub fn show(notification: &Notification) -> Result<()> {
    DesktopNotification::new()
        .appname("xretype")
        .summary(&notification.title)
        .body(&notification.body)
        .timeout(Timeout::Milliseconds(notification.timeout_ms))
        .show()
        .context("could not show desktop notification")?;
    Ok(())
}

pub fn notify_error(message: &str) -> Result<()> {
    show(&Notification {
        title: "xretype error".to_owned(),
        body: message.to_owned(),
        timeout_ms: 2500,
    })
}
