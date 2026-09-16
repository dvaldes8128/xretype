pub mod actions;
pub mod automation;
pub mod cli;
pub mod clipboard;
pub mod config;
pub mod input;
pub mod sources;
pub mod visual;

use anyhow::{Context, Result};

use crate::{
    actions::{Action, Runtime},
    cli::{Cli, Command, OverlayCommand},
    clipboard::Sensitivity,
    config::Config,
    sources::ValueSource,
    visual::{Notification, OverlayOperation, OverlayRequest},
};

pub fn run(cli: Cli) -> Result<()> {
    let config = Config::load(cli.config.as_deref())?;
    let notify_errors = config.feedback.notify_errors;
    let action = command_to_action(cli.command, &config)?;
    let mut runtime = Runtime::new(config);

    let result = runtime.execute(action);
    if let Err(error) = &result
        && notify_errors
    {
        let _ = visual::notify_error(&error.to_string());
    }
    result
}

fn command_to_action(command: Command, config: &Config) -> Result<Action> {
    match command {
        Command::Type(args) => Ok(Action::Type(args.value.resolve()?)),
        Command::Paste(args) => Ok(Action::Paste {
            text: args.value.resolve()?,
            sensitivity: Sensitivity::from_public(args.public),
        }),
        Command::Info(args) => {
            let path = sources::json::normalize_path(&args.path)?;
            let file = args.file.unwrap_or_else(|| config.info_file());
            let text = ValueSource::Json { file, path }
                .resolve()
                .context("could not resolve info value")?;

            if args.type_text {
                Ok(Action::Type(text))
            } else {
                Ok(Action::Paste {
                    text,
                    sensitivity: Sensitivity::from_public(args.public),
                })
            }
        }
        Command::Key(args) => Ok(Action::Key(args.key)),
        Command::Combo(args) => Ok(Action::Combo(args.keys)),
        Command::Sleep(args) => Ok(Action::Sleep(args.milliseconds)),
        Command::Notify(args) => Ok(Action::Notify(Notification {
            title: args.title,
            body: args.message,
            timeout_ms: args.timeout_ms,
        })),
        Command::Overlay(args) => {
            let request = match args.command {
                OverlayCommand::Show { name } => OverlayRequest {
                    operation: OverlayOperation::Show,
                    name: Some(name),
                },
                OverlayCommand::Hide { name } => OverlayRequest {
                    operation: OverlayOperation::Hide,
                    name,
                },
                OverlayCommand::Toggle { name } => OverlayRequest {
                    operation: OverlayOperation::Toggle,
                    name: Some(name),
                },
            };
            Ok(Action::Overlay(request))
        }
    }
}
