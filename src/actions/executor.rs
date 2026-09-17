use std::{thread, time::Duration};

use anyhow::Result;

use crate::{
    actions::Action,
    clipboard,
    config::{Config, InputBackendKind},
    input::{EnigoInput, InputBackend, YdotoolInput},
    sources::ValueSource,
    visual::{DesktopVisual, VisualBackend},
};

pub struct Runtime {
    config: Config,
    input: Option<Box<dyn InputBackend>>,
    visual: DesktopVisual,
}

impl Runtime {
    pub fn new(config: Config) -> Self {
        let visual = DesktopVisual::new(config.xremap_root());
        Self {
            config,
            input: None,
            visual,
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
            Action::Info {
                file,
                path,
                type_text,
                sensitivity,
            } => {
                let file = file.unwrap_or_else(|| self.config.info_file());
                let text = ValueSource::Json { file, path }.resolve()?;
                if type_text {
                    self.input()?.type_text(&text)
                } else {
                    let delay = Duration::from_millis(self.config.input.paste_delay_ms);
                    let serve_for = Duration::from_millis(self.config.input.clipboard_serve_ms);
                    clipboard::paste_with(self.input()?, text, sensitivity, delay, serve_for)
                }
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

    fn input(&mut self) -> Result<&mut dyn InputBackend> {
        if self.input.is_none() {
            let input: Box<dyn InputBackend> = match self.config.input.backend {
                InputBackendKind::Libei => Box::new(EnigoInput::new()?),
                InputBackendKind::Ydotool => Box::new(YdotoolInput::new()),
            };
            self.input = Some(input);
        }
        Ok(self.input.as_mut().expect("input was initialized").as_mut())
    }
}
