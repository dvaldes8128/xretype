use std::{collections::BTreeMap, path::PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};

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

    /// Execute locally instead of contacting xretyped.
    #[arg(long, global = true)]
    pub standalone: bool,

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
    /// Manage the native keyboard-layout overlay.
    Overlay(OverlayArgs),
    /// Run a named workflow from automations.yml.
    Run(RunArgs),
    /// Parse and validate the automation file without executing it.
    Validate,
    /// Inspect or reload the persistent service.
    Daemon(DaemonArgs),
    /// Generate or inspect the xremap integration.
    Xremap(XremapArgs),
    /// Internal native overlay renderer.
    #[command(name = "overlay-host", hide = true)]
    OverlayHost(OverlayHostArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    pub workflow: String,

    /// Bind a workflow parameter as NAME=VALUE. Repeat for multiple values.
    #[arg(long = "param", value_name = "NAME=VALUE", value_parser = parse_parameter)]
    pub parameters: Vec<(String, String)>,
}

impl RunArgs {
    pub fn parameter_map(self) -> anyhow::Result<BTreeMap<String, String>> {
        let mut values = BTreeMap::new();
        for (name, value) in self.parameters {
            if values.insert(name.clone(), value).is_some() {
                anyhow::bail!("parameter '{name}' was supplied more than once");
            }
        }
        Ok(values)
    }
}

fn parse_parameter(value: &str) -> Result<(String, String), String> {
    let (name, value) = value
        .split_once('=')
        .ok_or_else(|| "expected NAME=VALUE".to_owned())?;
    if name.is_empty() {
        return Err("parameter name cannot be empty".to_owned());
    }
    Ok((name.to_owned(), value.to_owned()))
}

#[derive(Debug, Args)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonCommand,
}

#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    Status,
    Reload,
}

#[derive(Debug, Args)]
pub struct XremapArgs {
    #[command(subcommand)]
    pub command: XremapCommand,
}

#[derive(Debug, Subcommand)]
pub enum XremapCommand {
    /// Regenerate marked blocks in config.yml.
    Generate(XremapGenerateArgs),
    /// List discovered layouts.
    Layouts(XremapRootArgs),
}

#[derive(Debug, Args)]
pub struct XremapRootArgs {
    /// Override the xremap configuration root.
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct XremapGenerateArgs {
    /// Override the xremap configuration root.
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Regenerate only one marked block.
    #[arg(long, value_enum)]
    pub only: Option<XremapSection>,

    /// Print the resulting complete config without writing it.
    #[arg(long, conflicts_with = "check")]
    pub dry_run: bool,

    /// Fail when generated content differs from config.yml.
    #[arg(long, conflicts_with = "dry_run")]
    pub check: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum XremapSection {
    Layouts,
    PersonalInfo,
}

#[derive(Debug, Args)]
pub struct OverlayHostArgs {
    pub layout: PathBuf,
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

    #[test]
    fn workflow_parameters_require_name_value_pairs() {
        assert!(Cli::try_parse_from(["xretype", "run", "demo", "--param", "name=value"]).is_ok());
        assert!(Cli::try_parse_from(["xretype", "run", "demo", "--param", "broken"]).is_err());
    }
}
