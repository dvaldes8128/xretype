//! Read-only environment diagnostics behind `xretype doctor`.
//!
//! [`observe`] gathers facts from the live session; [`evaluate`] turns them
//! into checks without touching the environment again. Keeping the two apart
//! lets the tests inject hostile facts, which matters because the test suite
//! runs headless and the real session is neither Wayland nor service-backed.

use std::{
    env,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    automation::WorkflowRegistry,
    config::{Config, InputBackendKind},
    daemon::{DaemonRequest, DaemonResponse, DaemonStatus, call, socket_path},
};

/// Outcome of a single diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Working as intended.
    Ok,
    /// Degraded, but xretype still functions.
    Warn,
    /// Broken, and the reason a feature will not work.
    Fail,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// One reported diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub status: Status,
    /// Short, stable identifier; safe to match on from scripts.
    pub name: &'static str,
    /// Human-readable reason. Never carries resolved action text.
    pub detail: String,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: Status::Ok,
            name,
            detail: detail.into(),
        }
    }

    fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: Status::Warn,
            name,
            detail: detail.into(),
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status: Status::Fail,
            name,
            detail: detail.into(),
        }
    }
}

/// Everything the evaluator needs, gathered once up front.
#[derive(Debug, Clone)]
pub struct Facts {
    /// Version of this binary, from the manifest.
    pub client_version: &'static str,
    /// `WAYLAND_DISPLAY`, absent on X11 or a headless session.
    pub wayland_display: Option<String>,
    /// `XDG_RUNTIME_DIR` of the login session.
    pub xdg_runtime_dir: Option<PathBuf>,
    /// Whether the daemon socket is on disk.
    pub socket_exists: bool,
    /// Configured input backend, or the built-in default.
    pub backend: InputBackendKind,
    /// Parsed configuration, or the parse error.
    pub config: Result<Config, String>,
    /// Workflow count, or the automation file's parse error. `None` when the
    /// configuration did not parse, so the automation file was never resolved.
    pub automations: Option<Result<usize, String>>,
    /// Daemon status, or why it could not be reached.
    pub daemon: Result<DaemonStatus, String>,
}

/// Read the live session and the daemon. Never fails: every problem becomes a
/// fact so `doctor` can report several at once instead of stopping at the first.
pub fn observe(config_path: Option<&Path>) -> Facts {
    let config = Config::load(config_path).map_err(|error| format!("{error:#}"));

    // With a broken config the automation path is unknown, so the workflow
    // check is skipped rather than run against a file the user may not use.
    let automations = config.as_ref().ok().map(|config| {
        WorkflowRegistry::load(&config.automation_file())
            .map(|registry| registry.workflow_count())
            .map_err(|error| format!("{error:#}"))
    });

    let daemon = match call(DaemonRequest::Status) {
        Ok(DaemonResponse::Status(status)) => Ok(status),
        Ok(_) => Err("daemon returned an unexpected response".to_owned()),
        Err(error) => Err(format!("{error:#}")),
    };

    Facts {
        client_version: env!("CARGO_PKG_VERSION"),
        wayland_display: env::var("WAYLAND_DISPLAY").ok(),
        xdg_runtime_dir: env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        socket_exists: socket_path().exists(),
        backend: config
            .as_ref()
            .map(|config| config.input.backend)
            .unwrap_or_default(),
        config,
        automations,
        daemon,
    }
}

/// Turn observed facts into checks. Pure: the same facts always yield the same
/// report, which is what makes the suite able to cover the broken paths.
pub fn evaluate(facts: &Facts) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(session_check(facts));
    checks.push(runtime_directory_check(facts));
    checks.push(config_check(facts));

    // Report workflows only when the config parsed and the file was actually
    // read. A broken config leaves the automation path unresolved, and a
    // second failure against a possibly wrong file would mislead.
    if let (Ok(_), Some(automations)) = (&facts.config, &facts.automations) {
        checks.push(automations_check(automations));
    }

    checks.push(socket_check(facts));
    checks.push(daemon_check(facts));

    // Version and reload state only make sense against a daemon that answered.
    if let Ok(status) = &facts.daemon {
        checks.push(version_check(facts, status));
        checks.push(reload_check(status));
    }

    checks
}

fn session_check(facts: &Facts) -> Check {
    match (facts.backend, &facts.wayland_display) {
        (InputBackendKind::Libei, None) => Check::fail(
            "session",
            "not a Wayland session; the libei backend needs WAYLAND_DISPLAY",
        ),
        (InputBackendKind::Libei, Some(display)) => {
            Check::ok("session", format!("wayland ({display})"))
        }
        (InputBackendKind::Ydotool, None) => Check::ok(
            "session",
            "no Wayland display; ydotool injects through uinput",
        ),
        (InputBackendKind::Ydotool, Some(display)) => {
            Check::ok("session", format!("wayland ({display})"))
        }
    }
}

fn runtime_directory_check(facts: &Facts) -> Check {
    match &facts.xdg_runtime_dir {
        Some(directory) => Check::ok("runtime directory", directory.display().to_string()),
        None => Check::warn(
            "runtime directory",
            "XDG_RUNTIME_DIR is unset; the daemon falls back to /tmp/xretype-runtime-UID",
        ),
    }
}

fn config_check(facts: &Facts) -> Check {
    match &facts.config {
        Ok(_) => Check::ok("config", "parsed"),
        Err(error) => Check::fail("config", error.clone()),
    }
}

fn automations_check(automations: &Result<usize, String>) -> Check {
    match automations {
        Ok(0) => Check::warn(
            "automations",
            "no workflows are defined; `xretype validate` reads the automation file",
        ),
        Ok(count) => Check::ok("automations", format!("{count} workflow(s)")),
        Err(error) => Check::fail("automations", error.clone()),
    }
}

fn socket_check(facts: &Facts) -> Check {
    if facts.socket_exists {
        Check::ok("daemon socket", socket_path().display().to_string())
    } else {
        Check::warn(
            "daemon socket",
            "not found; start the user service or pass --standalone",
        )
    }
}

fn daemon_check(facts: &Facts) -> Check {
    match &facts.daemon {
        Ok(status) => Check::ok("daemon", format!("responding (pid {})", status.pid)),
        Err(error) => Check::warn(
            "daemon",
            format!("{error}; check `systemctl --user status xretyped.service`"),
        ),
    }
}

fn version_check(facts: &Facts, status: &DaemonStatus) -> Check {
    if status.version == facts.client_version {
        Check::ok("daemon version", facts.client_version.to_owned())
    } else {
        Check::fail(
            "daemon version",
            format!(
                "client {}, daemon {}; restart the user service to pick up the new build",
                facts.client_version, status.version
            ),
        )
    }
}

fn reload_check(status: &DaemonStatus) -> Check {
    match &status.last_reload_error {
        Some(error) => Check::warn(
            "last reload error",
            format!("the loaded workflows are stale: {error}"),
        ),
        None => Check::ok("last reload error", "none"),
    }
}

/// Whether any check failed. Warns alone do not fail the command.
pub fn has_failures(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.status == Status::Fail)
}

/// Render the report as aligned `status name detail` rows plus a summary.
pub fn write_report<W: Write>(out: &mut W, checks: &[Check]) -> io::Result<()> {
    let width = checks
        .iter()
        .map(|check| check.name.len())
        .max()
        .unwrap_or(0);

    for check in checks {
        writeln!(
            out,
            "{:<5} {:<width$}  {}",
            check.status.label(),
            check.name,
            check.detail,
            width = width
        )?;
    }

    let count = |status: Status| checks.iter().filter(|check| check.status == status).count();
    writeln!(
        out,
        "\n{} ok, {} warn, {} fail",
        count(Status::Ok),
        count(Status::Warn),
        count(Status::Fail)
    )
}

/// CLI entry for `xretype doctor`: report, then fail the command when a check
/// failed so scripts can branch on the exit status.
pub fn run(config_path: Option<&Path>) -> Result<()> {
    let checks = evaluate(&observe(config_path));
    let mut stdout = io::stdout().lock();
    write_report(&mut stdout, &checks).context("could not write the doctor report")?;

    if has_failures(&checks) {
        anyhow::bail!("one or more checks failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A healthy session: Wayland, a runtime directory, a parseable config
    /// with workflows, and a daemon reporting the same build we are.
    fn healthy_facts() -> Facts {
        Facts {
            client_version: "0.1.0",
            wayland_display: Some("wayland-0".to_owned()),
            xdg_runtime_dir: Some(PathBuf::from("/run/user/1000")),
            socket_exists: true,
            backend: InputBackendKind::Libei,
            config: Ok(Config::default()),
            automations: Some(Ok(2)),
            daemon: Ok(DaemonStatus {
                version: "0.1.0".to_owned(),
                pid: 3633,
                ..DaemonStatus::default()
            }),
        }
    }

    fn check_named<'a>(checks: &'a [Check], name: &str) -> &'a Check {
        checks
            .iter()
            .find(|check| check.name == name)
            .unwrap_or_else(|| panic!("no {name:?} check in report"))
    }

    #[test]
    fn healthy_environment_passes_every_check() {
        let checks = evaluate(&healthy_facts());

        assert_eq!(checks.len(), 8);
        assert!(
            checks.iter().all(|check| check.status == Status::Ok),
            "expected all ok, got {checks:#?}"
        );
        assert!(!has_failures(&checks));
    }

    #[test]
    fn broken_config_fails_and_omits_the_workflow_check() {
        let mut facts = healthy_facts();
        facts.config = Err("TOML parse error at line 1".to_owned());

        let checks = evaluate(&facts);

        assert_eq!(check_named(&checks, "config").status, Status::Fail);
        // The automation path was never resolved, so reporting a second
        // failure against a possibly wrong file would mislead the operator.
        assert!(
            !checks.iter().any(|check| check.name == "automations"),
            "workflow check must be skipped when the config is unusable"
        );
        assert!(has_failures(&checks));
    }

    #[test]
    fn parseable_config_without_workflows_warns_instead_of_failing() {
        let mut facts = healthy_facts();
        facts.automations = Some(Ok(0));

        let checks = evaluate(&facts);

        assert_eq!(check_named(&checks, "automations").status, Status::Warn);
        assert!(!has_failures(&checks));
    }

    #[test]
    fn broken_automation_file_fails() {
        let mut facts = healthy_facts();
        facts.automations = Some(Err(
            "workflow 'x' must contain at least one action".to_owned()
        ));

        let checks = evaluate(&facts);

        assert_eq!(check_named(&checks, "automations").status, Status::Fail);
        assert!(has_failures(&checks));
    }

    #[test]
    fn client_and_daemon_version_mismatch_fails() {
        let mut facts = healthy_facts();
        facts.daemon = Ok(DaemonStatus {
            version: "0.0.9".to_owned(),
            ..DaemonStatus::default()
        });

        let checks = evaluate(&facts);

        let version = check_named(&checks, "daemon version");
        assert_eq!(version.status, Status::Fail);
        assert!(version.detail.contains("client 0.1.0"));
        assert!(version.detail.contains("daemon 0.0.9"));
        assert!(has_failures(&checks));
    }

    #[test]
    fn matching_versions_pass() {
        let checks = evaluate(&healthy_facts());

        let version = check_named(&checks, "daemon version");
        assert_eq!(version.status, Status::Ok);
        assert_eq!(version.detail, "0.1.0");
    }

    #[test]
    fn libei_needs_wayland() {
        let mut facts = healthy_facts();
        facts.wayland_display = None;
        facts.backend = InputBackendKind::Libei;

        let checks = evaluate(&facts);

        assert_eq!(check_named(&checks, "session").status, Status::Fail);
        assert!(has_failures(&checks));
    }

    #[test]
    fn ydotool_does_not_need_wayland() {
        let mut facts = healthy_facts();
        facts.wayland_display = None;
        facts.backend = InputBackendKind::Ydotool;

        let checks = evaluate(&facts);

        assert_eq!(check_named(&checks, "session").status, Status::Ok);
        assert!(!has_failures(&checks));
    }

    #[test]
    fn missing_runtime_directory_warns_because_a_fallback_exists() {
        let mut facts = healthy_facts();
        facts.xdg_runtime_dir = None;

        let checks = evaluate(&facts);

        assert_eq!(
            check_named(&checks, "runtime directory").status,
            Status::Warn
        );
        assert!(!has_failures(&checks));
    }

    #[test]
    fn unreachable_daemon_warns_and_skips_daemon_only_checks() {
        let mut facts = healthy_facts();
        facts.socket_exists = false;
        facts.daemon = Err("could not connect to xretyped".to_owned());

        let checks = evaluate(&facts);

        assert_eq!(check_named(&checks, "daemon socket").status, Status::Warn);
        assert_eq!(check_named(&checks, "daemon").status, Status::Warn);
        // A daemon that never answered cannot be version-compared, and the
        // `daemon` row already explains why.
        assert!(
            !checks.iter().any(|check| check.name == "daemon version"),
            "version check must not run without a daemon response"
        );
        assert!(
            !checks.iter().any(|check| check.name == "last reload error"),
            "reload check must not run without a daemon response"
        );
        // Degrading must not break the command: warns are not failures.
        assert!(!has_failures(&checks));
    }

    #[test]
    fn a_stale_daemon_reload_is_a_warning() {
        let mut facts = healthy_facts();
        facts.daemon = Ok(DaemonStatus {
            version: "0.1.0".to_owned(),
            last_reload_error: Some("workflow 'x' must contain at least one action".to_owned()),
            ..DaemonStatus::default()
        });

        let checks = evaluate(&facts);

        assert_eq!(
            check_named(&checks, "last reload error").status,
            Status::Warn
        );
        assert!(!has_failures(&checks));
    }

    #[test]
    fn only_failures_break_the_command() {
        assert!(!has_failures(&[]));
        assert!(
            !has_failures(&[Check::warn("a", "detail"), Check::ok("b", "detail")]),
            "warnings must not fail doctor"
        );
        assert!(has_failures(&[
            Check::ok("a", "detail"),
            Check::fail("b", "detail")
        ]));
    }

    #[test]
    fn report_aligns_rows_and_summarizes() {
        let checks = vec![
            Check::ok("session", "wayland (wayland-0)"),
            Check::warn("daemon socket", "not found"),
            Check::fail("config", "TOML parse error"),
        ];
        let mut out = Vec::new();
        write_report(&mut out, &checks).unwrap();
        let report = String::from_utf8(out).unwrap();

        assert!(report.contains("ok    session"));
        assert!(report.contains("warn  daemon socket"));
        assert!(report.contains("fail  config"));
        assert!(report.contains("1 ok, 1 warn, 1 fail"));
        // Rows, one blank separator, then the summary.
        assert_eq!(report.lines().count(), 5);
        assert!(report.contains("\n\n1 ok, 1 warn, 1 fail\n"));
    }

    #[test]
    fn report_of_an_empty_set_still_summarizes() {
        let mut out = Vec::new();
        write_report(&mut out, &[]).unwrap();

        assert_eq!(String::from_utf8(out).unwrap(), "\n0 ok, 0 warn, 0 fail\n");
    }
}
