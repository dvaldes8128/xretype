use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::sources::ValueSource;

#[derive(Debug, Parser)]
#[command(
    name = "xretype",
    version,
    about = "Typing automation and protected paste helper for xremap"
)]
pub struct Cli {
    /// Read settings from this TOML file.
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Type text directly through Enigo/libei.
    Type(TextArgs),
    /// Put text on the clipboard and inject Ctrl+V.
    Paste(PasteArgs),
    /// Resolve a JSON path and paste the scalar value.
    Info(InfoArgs),
    /// Click one named key.
    Key(KeyArgs),
    /// Press a key combination, such as `ctrl shift t`.
    Combo(ComboArgs),
    /// Pause for a number of milliseconds.
    Sleep(SleepArgs),
    /// Show a desktop notification.
    Notify(NotifyArgs),
    /// Manage a visual hint overlay (backend placeholder in v0.1).
    Overlay(OverlayArgs),
}

#[derive(Debug, Args)]
pub struct TextArgs {
    #[command(flatten)]
    pub value: ValueArgs,
}

#[derive(Debug, Args)]
pub struct PasteArgs {
    #[command(flatten)]
    pub value: ValueArgs,

    /// Allow the value to be retained in normal clipboard history.
    #[arg(long)]
    pub public: bool,
}

#[derive(Debug, Args)]
pub struct ValueArgs {
    /// Literal text. Omit this when using --stdin.
    #[arg(
        value_name = "TEXT",
        required_unless_present = "stdin",
        conflicts_with = "stdin"
    )]
    pub text: Option<String>,

    /// Read the text from standard input.
    #[arg(long)]
    pub stdin: bool,
}

impl ValueArgs {
    pub fn resolve(self) -> anyhow::Result<String> {
        if self.stdin {
            ValueSource::Stdin.resolve()
        } else {
            ValueSource::Literal(self.text.unwrap_or_default()).resolve()
        }
    }
}

#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Override the configured personal-information JSON file.
    #[arg(long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Type directly instead of using the clipboard.
    #[arg(long = "type", conflicts_with = "public")]
    pub type_text: bool,

    /// Allow the resolved value to be retained in clipboard history.
    #[arg(long)]
    pub public: bool,

    /// Nested keys, as separate arguments or a dotted path.
    #[arg(required = true, value_name = "PATH")]
    pub path: Vec<String>,
}

#[derive(Debug, Args)]
pub struct KeyArgs {
    pub key: String,
}

#[derive(Debug, Args)]
pub struct ComboArgs {
    #[arg(required = true, num_args = 1.., value_name = "KEY")]
    pub keys: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SleepArgs {
    pub milliseconds: u64,
}

#[derive(Debug, Args)]
pub struct NotifyArgs {
    pub message: String,

    #[arg(long, default_value = "xretype")]
    pub title: String,

    #[arg(long, default_value_t = 1500)]
    pub timeout_ms: u32,
}

#[derive(Debug, Args)]
pub struct OverlayArgs {
    #[command(subcommand)]
    pub command: OverlayCommand,
}

#[derive(Debug, Subcommand)]
pub enum OverlayCommand {
    Show { name: String },
    Hide { name: Option<String> },
    Toggle { name: String },
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn clap_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn paste_is_sensitive_unless_public_is_present() {
        let cli = Cli::try_parse_from(["xretype", "paste", "secret"]).unwrap();
        let Command::Paste(args) = cli.command else {
            panic!("expected paste command");
        };
        assert!(!args.public);

        let cli = Cli::try_parse_from(["xretype", "paste", "--public", "ordinary"]).unwrap();
        let Command::Paste(args) = cli.command else {
            panic!("expected paste command");
        };
        assert!(args.public);
    }

    #[test]
    fn input_must_be_literal_or_stdin_but_not_both() {
        assert!(Cli::try_parse_from(["xretype", "type"]).is_err());
        assert!(Cli::try_parse_from(["xretype", "type", "text", "--stdin"]).is_err());
        assert!(Cli::try_parse_from(["xretype", "type", "--stdin"]).is_ok());
    }
}
