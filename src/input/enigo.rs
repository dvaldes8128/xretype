use anyhow::{Context, Result, bail};
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};

use super::InputBackend;

pub struct EnigoInput {
    inner: Enigo,
}

impl EnigoInput {
    pub fn new() -> Result<Self> {
        let inner = Enigo::new(&Settings::default())
            .context("could not establish an Enigo/libei input session")?;
        Ok(Self { inner })
    }
}

impl InputBackend for EnigoInput {
    fn type_text(&mut self, text: &str) -> Result<()> {
        self.inner.text(text).context("could not inject text")
    }

    fn key(&mut self, key: &str) -> Result<()> {
        let key = parse_key(key)?;
        self.inner.key(key, Click).context("could not inject key")
    }

    fn combo(&mut self, keys: &[String]) -> Result<()> {
        let parsed = keys
            .iter()
            .map(|key| parse_key(key))
            .collect::<Result<Vec<_>>>()?;
        let Some((last, modifiers)) = parsed.split_last() else {
            bail!("a key combination cannot be empty");
        };

        let mut pressed = Vec::with_capacity(modifiers.len());
        for key in modifiers {
            if let Err(error) = self.inner.key(*key, Press) {
                for pressed_key in pressed.iter().rev() {
                    let _ = self.inner.key(*pressed_key, Release);
                }
                return Err(error).context("could not press key combination");
            }
            pressed.push(*key);
        }

        let click_result = self.inner.key(*last, Click);
        let mut release_error = None;
        for key in pressed.iter().rev() {
            if let Err(error) = self.inner.key(*key, Release)
                && release_error.is_none()
            {
                release_error = Some(error);
            }
        }

        click_result.context("could not click final key in combination")?;
        if let Some(error) = release_error {
            return Err(error).context("could not release key combination");
        }
        Ok(())
    }
}

pub fn parse_key(value: &str) -> Result<Key> {
    let normalized = value.trim().to_ascii_lowercase().replace(['_', '-'], "");
    let key = match normalized.as_str() {
        "alt" | "option" => Key::Alt,
        "backspace" => Key::Backspace,
        "capslock" => Key::CapsLock,
        "ctrl" | "control" => Key::Control,
        "delete" | "del" => Key::Delete,
        "down" | "downarrow" => Key::DownArrow,
        "end" => Key::End,
        "enter" | "return" => Key::Return,
        "esc" | "escape" => Key::Escape,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "f13" => Key::F13,
        "f14" => Key::F14,
        "f15" => Key::F15,
        "f16" => Key::F16,
        "f17" => Key::F17,
        "f18" => Key::F18,
        "f19" => Key::F19,
        "f20" => Key::F20,
        "f21" => Key::F21,
        "f22" => Key::F22,
        "f23" => Key::F23,
        "f24" => Key::F24,
        "home" => Key::Home,
        "insert" | "ins" => Key::Insert,
        "left" | "leftarrow" => Key::LeftArrow,
        "meta" | "super" | "win" | "windows" => Key::Meta,
        "pagedown" | "pgdown" | "pgdn" => Key::PageDown,
        "pageup" | "pgup" => Key::PageUp,
        "right" | "rightarrow" => Key::RightArrow,
        "shift" => Key::Shift,
        "space" => Key::Space,
        "tab" => Key::Tab,
        "up" | "uparrow" => Key::UpArrow,
        "volumedown" => Key::VolumeDown,
        "volumemute" | "mute" => Key::VolumeMute,
        "volumeup" => Key::VolumeUp,
        _ => {
            let mut characters = value.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => Key::Unicode(character),
                _ => bail!("unknown key '{value}'"),
            }
        }
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use enigo::Key;

    use super::parse_key;

    #[test]
    fn parses_aliases_and_unicode() {
        assert_eq!(parse_key("ctrl").unwrap(), Key::Control);
        assert_eq!(parse_key("page-down").unwrap(), Key::PageDown);
        assert_eq!(parse_key("é").unwrap(), Key::Unicode('é'));
        assert!(parse_key("definitely-not-a-key").is_err());
    }
}
