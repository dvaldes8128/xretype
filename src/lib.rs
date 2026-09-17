pub mod actions;
pub mod automation;
pub mod cli;
pub mod clipboard;
pub mod config;
pub mod daemon;
pub mod input;
pub mod sources;
pub mod visual;
pub mod xremap;

use anyhow::Result;

use crate::{
    actions::{Action, Runtime},
    automation::WorkflowRegistry,
    cli::{Cli, Command, DaemonCommand, OverlayCommand, XremapCommand, XremapSection},
    clipboard::Sensitivity,
    config::Config,
    visual::{Notification, OverlayOperation, OverlayRequest},
};

pub fn run(cli: Cli) -> Result<()> {
    let config = Config::load(cli.config.as_deref())?;
    let notify_errors = config.feedback.notify_errors;
    let result = dispatch(cli.command, cli.standalone, config);
    if let Err(error) = &result
        && notify_errors
    {
        let _ = visual::notify_error(&error.to_string());
    }
    result
}

fn dispatch(command: Command, standalone: bool, config: Config) -> Result<()> {
    match command {
        Command::Run(args) => {
            let workflow = args.workflow.clone();
            let parameters = args.parameter_map()?;
            if standalone {
                let registry = WorkflowRegistry::load(&config.automation_file())?;
                registry.execute_strings(&workflow, &parameters, &mut Runtime::new(config))
            } else {
                daemon::call(daemon::DaemonRequest::Run {
                    workflow,
                    parameters,
                })?;
                Ok(())
            }
        }
        Command::Validate => {
            let registry = WorkflowRegistry::load(&config.automation_file())?;
            println!("valid: {} workflow(s)", registry.workflow_count());
            Ok(())
        }
        Command::Daemon(args) => {
            if standalone {
                anyhow::bail!("--standalone cannot be used with daemon commands");
            }
            let request = match args.command {
                DaemonCommand::Status => daemon::DaemonRequest::Status,
                DaemonCommand::Reload => daemon::DaemonRequest::Reload,
            };
            match daemon::call(request)? {
                daemon::DaemonResponse::Done => println!("ok"),
                daemon::DaemonResponse::Status(status) => print_status(&status),
            }
            Ok(())
        }
        Command::Xremap(args) => {
            if standalone {
                anyhow::bail!("--standalone is not meaningful for xremap generation commands");
            }
            match args.command {
                XremapCommand::Generate(args) => {
                    let options = xremap::GenerateOptions {
                        root: args.root,
                        section: args.only.map(|section| match section {
                            XremapSection::Layouts => xremap::GenerateSection::Layouts,
                            XremapSection::PersonalInfo => xremap::GenerateSection::PersonalInfo,
                        }),
                        dry_run: args.dry_run,
                        check: args.check,
                    };
                    let changed = xremap::generate(&config, &options)?;
                    if !args.dry_run && !args.check {
                        println!("{}", if changed { "updated" } else { "unchanged" });
                    }
                    Ok(())
                }
                XremapCommand::Layouts(args) => {
                    let root = args.root.unwrap_or_else(|| config.xremap_root());
                    for layout in xremap::load_layouts(&root)? {
                        println!(
                            "{}  {:<16} mode={:<18} entries={:>3}  file={}",
                            layout.selector,
                            layout.name,
                            layout.mode,
                            layout.entries.len(),
                            layout.path.display()
                        );
                    }
                    Ok(())
                }
            }
        }
        Command::OverlayHost(args) => visual::run_overlay_host(&args.layout),
        command => {
            let action = command_to_action(command)?;
            if standalone {
                Runtime::new(config).execute(action)
            } else {
                daemon::call(daemon::DaemonRequest::Execute(action))?;
                Ok(())
            }
        }
    }
}

fn print_status(status: &daemon::DaemonStatus) {
    println!("xretyped {} (pid {})", status.version, status.pid);
    println!(
        "configuration generation: {} ({} workflows)",
        status.config_generation, status.workflow_count
    );
    println!("queue depth: {}", status.queue_depth);
    println!(
        "active workflow: {}",
        status.active_workflow.as_deref().unwrap_or("none")
    );
    println!(
        "active overlay: {}",
        status.active_overlay.as_deref().unwrap_or("none")
    );
    if let Some(error) = &status.last_reload_error {
        println!("last reload error: {error}");
    }
}

fn command_to_action(command: Command) -> Result<Action> {
    match command {
        Command::Type(args) => Ok(Action::Type(args.value.resolve()?)),
        Command::Paste(args) => Ok(Action::Paste {
            text: args.value.resolve()?,
            sensitivity: Sensitivity::from_public(args.public),
        }),
        Command::Info(args) => {
            let path = sources::json::normalize_path(&args.path)?;
            Ok(Action::Info {
                file: args.file,
                path,
                type_text: args.type_text,
                sensitivity: Sensitivity::from_public(args.public),
            })
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
        Command::Run(_)
        | Command::Validate
        | Command::Daemon(_)
        | Command::Xremap(_)
        | Command::OverlayHost(_) => {
            anyhow::bail!("command is not a primitive action")
        }
    }
}
