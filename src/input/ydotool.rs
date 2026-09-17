use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};

use super::InputBackend;

#[derive(Default)]
pub struct YdotoolInput;

impl YdotoolInput {
    pub fn new() -> Self {
        Self
    }

    fn run_key_sequence(keys: &[String]) -> Result<()> {
        let events = key_events(keys)?;
        let status = Command::new("ydotool")
            .arg("key")
            .args(["--key-delay", "0"])
            .args(events)
            .status()
            .context("could not launch ydotool key")?;

        if !status.success() {
            bail!("ydotool key exited with status {status}");
        }
        Ok(())
    }
}

impl InputBackend for YdotoolInput {
    fn type_text(&mut self, text: &str) -> Result<()> {
        let mut child = Command::new("ydotool")
            .args(["type", "--key-delay", "0", "--file", "-"])
            .stdin(Stdio::piped())
            .spawn()
            .context("could not launch ydotool type")?;

        child
            .stdin
            .take()
            .context("ydotool type stdin was unavailable")?
            .write_all(text.as_bytes())
            .context("could not send text to ydotool")?;

        let status = child.wait().context("could not wait for ydotool type")?;
        if !status.success() {
            bail!("ydotool type exited with status {status}");
        }
        Ok(())
    }

    fn key(&mut self, key: &str) -> Result<()> {
        Self::run_key_sequence(&[key.to_owned()])
    }

    fn combo(&mut self, keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            bail!("a key combination cannot be empty");
        }
        Self::run_key_sequence(keys)
    }
}

fn key_events(keys: &[String]) -> Result<Vec<String>> {
    let codes = keys
        .iter()
        .map(|key| keycode(key))
        .collect::<Result<Vec<_>>>()?;
    let mut events = Vec::with_capacity(codes.len() * 2);
    events.extend(codes.iter().map(|code| format!("{code}:1")));
    events.extend(codes.iter().rev().map(|code| format!("{code}:0")));
    Ok(events)
}

fn keycode(value: &str) -> Result<u16> {
    let trimmed = value.trim();
    if trimmed.chars().count() == 1 {
        let character = trimmed
            .chars()
            .next()
            .expect("single-character value was checked");
        if let Some(code) = ascii_keycode(character) {
            return Ok(code);
        }
    }

    let normalized = trimmed.to_ascii_lowercase().replace(['_', '-'], "");
    let code = match normalized.as_str() {
        "alt" | "option" => 56,
        "backspace" => 14,
        "capslock" => 58,
        "ctrl" | "control" => 29,
        "delete" | "del" => 111,
        "down" | "downarrow" => 108,
        "end" => 107,
        "enter" | "return" => 28,
        "esc" | "escape" => 1,
        "f1" => 59,
        "f2" => 60,
        "f3" => 61,
        "f4" => 62,
        "f5" => 63,
        "f6" => 64,
        "f7" => 65,
        "f8" => 66,
        "f9" => 67,
        "f10" => 68,
        "f11" => 87,
        "f12" => 88,
        "f13" => 183,
        "f14" => 184,
        "f15" => 185,
        "f16" => 186,
        "f17" => 187,
        "f18" => 188,
        "f19" => 189,
        "f20" => 190,
        "f21" => 191,
        "f22" => 192,
        "f23" => 193,
        "f24" => 194,
        "home" => 102,
        "insert" | "ins" => 110,
        "left" | "leftarrow" => 105,
        "meta" | "super" | "win" | "windows" => 125,
        "pagedown" | "pgdown" | "pgdn" => 109,
        "pageup" | "pgup" => 104,
        "right" | "rightarrow" => 106,
        "shift" => 42,
        "space" => 57,
        "tab" => 15,
        "up" | "uparrow" => 103,
        "volumedown" => 114,
        "volumemute" | "mute" => 113,
        "volumeup" => 115,
        _ => bail!("unknown ydotool key '{value}'"),
    };
    Ok(code)
}

fn ascii_keycode(character: char) -> Option<u16> {
    let code = match character.to_ascii_lowercase() {
        '1' => 2,
        '2' => 3,
        '3' => 4,
        '4' => 5,
        '5' => 6,
        '6' => 7,
        '7' => 8,
        '8' => 9,
        '9' => 10,
        '0' => 11,
        '-' => 12,
        '=' => 13,
        'q' => 16,
        'w' => 17,
        'e' => 18,
        'r' => 19,
        't' => 20,
        'y' => 21,
        'u' => 22,
        'i' => 23,
        'o' => 24,
        'p' => 25,
        '[' => 26,
        ']' => 27,
        'a' => 30,
        's' => 31,
        'd' => 32,
        'f' => 33,
        'g' => 34,
        'h' => 35,
        'j' => 36,
        'k' => 37,
        'l' => 38,
        ';' => 39,
        '\'' => 40,
        '`' => 41,
        '\\' => 43,
        'z' => 44,
        'x' => 45,
        'c' => 46,
        'v' => 47,
        'b' => 48,
        'n' => 49,
        'm' => 50,
        ',' => 51,
        '.' => 52,
        '/' => 53,
        ' ' => 57,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::{key_events, keycode};

    #[test]
    fn builds_press_then_reverse_release_sequence() {
        let keys = vec!["ctrl".to_owned(), "v".to_owned()];
        assert_eq!(key_events(&keys).unwrap(), ["29:1", "47:1", "47:0", "29:0"]);
    }

    #[test]
    fn maps_named_and_ascii_keys() {
        assert_eq!(keycode("enter").unwrap(), 28);
        assert_eq!(keycode("V").unwrap(), 47);
        assert!(keycode("é").is_err());
    }
}
