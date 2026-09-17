use std::{
    ffi::OsString,
    fs,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "xretyped",
    version,
    about = "Persistent xretype automation service"
)]
struct Args {
    /// Read settings from this TOML file.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
}

fn main() {
    configure_wayland_display();

    let args = Args::parse();
    let result =
        xretype::config::Config::load(args.config.as_deref()).and_then(xretype::daemon::serve);
    if let Err(error) = result {
        eprintln!("xretyped: {error:#}");
        std::process::exit(1);
    }
}

/// Desktop sessions can import `WAYLAND_DISPLAY` into the systemd user manager
/// just after services ordered behind `graphical-session.target` have started.
/// Recover the compositor socket in that narrow login race so clipboard code
/// does not incorrectly fall back to X11 for the daemon's entire lifetime.
fn configure_wayland_display() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return;
    }
    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return;
    };
    let Some(display) = find_wayland_display(Path::new(&runtime_dir)) else {
        return;
    };

    // SAFETY: this is the first operation in `main`, before xretyped creates
    // its worker, watcher, or connection threads. No other thread can read the
    // process environment concurrently.
    unsafe { std::env::set_var("WAYLAND_DISPLAY", display) };
}

fn find_wayland_display(runtime_dir: &Path) -> Option<OsString> {
    let displays = fs::read_dir(runtime_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_socket()))
        .map(|entry| entry.file_name())
        .filter(|name| name.as_encoded_bytes().starts_with(b"wayland-"))
        .collect::<Vec<_>>();
    choose_wayland_display(displays)
}

fn choose_wayland_display(mut displays: Vec<OsString>) -> Option<OsString> {
    displays.sort();

    displays
        .iter()
        .find(|name| name.as_encoded_bytes() == b"wayland-0")
        .cloned()
        .or_else(|| displays.into_iter().next())
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{choose_wayland_display, find_wayland_display};

    fn temporary_directory() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("xretyped-test-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn ignores_regular_wayland_files() {
        let directory = temporary_directory();
        fs::write(directory.join("wayland-0.lock"), []).unwrap();
        fs::write(directory.join("wayland-1"), []).unwrap();

        assert_eq!(find_wayland_display(&directory), None);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn prefers_the_conventional_display_when_several_exist() {
        assert_eq!(
            choose_wayland_display(vec![
                OsString::from("wayland-2"),
                OsString::from("wayland-0"),
                OsString::from("wayland-1"),
            ]),
            Some(OsString::from("wayland-0"))
        );
    }
}
