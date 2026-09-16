use serde::Deserialize;

/// Configuration shape reserved for a future named-sequence runner.
///
/// v0.1 deliberately does not expose a `run` command or execute these values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceDefinition {
    pub name: String,
    pub actions: Vec<SequenceAction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SequenceAction {
    Type {
        text: String,
    },
    Paste {
        text: String,
        #[serde(default = "sensitive_by_default")]
        sensitive: bool,
    },
    Key {
        key: String,
    },
    Combo {
        keys: Vec<String>,
    },
    Sleep {
        milliseconds: u64,
    },
    Notify {
        title: String,
        body: String,
    },
    Overlay {
        name: String,
    },
}

const fn sensitive_by_default() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{SequenceAction, SequenceDefinition};

    #[test]
    fn future_sequence_pastes_are_sensitive_by_default() {
        let definition: SequenceDefinition = toml::from_str(
            r#"
name = "login"

[[actions]]
action = "paste"
text = "secret"
"#,
        )
        .unwrap();

        let SequenceAction::Paste { sensitive, .. } = &definition.actions[0] else {
            panic!("expected paste action");
        };
        assert!(*sensitive);
    }
}
