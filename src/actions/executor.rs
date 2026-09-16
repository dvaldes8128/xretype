use std::{thread, time::Duration};

use anyhow::Result;

use crate::{
    actions::Action,
    clipboard,
    config::Config,
    input::{EnigoInput, InputBackend},
    visual::{DesktopVisual, VisualBackend},
};

pub struct Runtime {
    config: Config,
    input: Option<EnigoInput>,
    visual: DesktopVisual,
}

impl Runtime {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            input: None,
            visual: DesktopVisual,
        }
    }

    pub fn execute(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Type(text) => self.input()?.type_text(&text),
            Action::Paste { text, sensitivity } => {
                let delay = Duration::from_millis(self.config.input.paste_delay_ms);
                let serve_for = Duration::from_millis(self.config.input.clipboard_serve_ms);
                clipboard::paste_with(self.input()?, text, sensitivity, delay, serve_for)
            }
            Action::Key(key) => self.input()?.key(&key),
            Action::Combo(keys) => self.input()?.combo(&keys),
            Action::Sleep(milliseconds) => {
                thread::sleep(Duration::from_millis(milliseconds));
                Ok(())
            }
            Action::Notify(notification) => self.visual.notify(&notification),
            Action::Overlay(request) => self.visual.overlay(&request),
        }
    }

    fn input(&mut self) -> Result<&mut EnigoInput> {
        if self.input.is_none() {
            self.input = Some(EnigoInput::new()?);
        }
        Ok(self.input.as_mut().expect("input was initialized"))
    }
}
