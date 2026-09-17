use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
};

use anyhow::{Context, Result, bail};
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, CenterBox, CssProvider, Frame, Label,
    Orientation, STYLE_PROVIDER_PRIORITY_APPLICATION, gdk, gio, glib, prelude::*,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde::{Deserialize, Serialize};

use crate::{daemon::socket_path, xremap::Layout};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayOperation {
    Show,
    Hide,
    Toggle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayView {
    #[default]
    Base,
    Prefix,
}

impl OverlayView {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Prefix => "prefix",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayRequest {
    pub operation: OverlayOperation,
    pub name: Option<String>,
    #[serde(default)]
    pub view: OverlayView,
}

#[derive(Debug, Serialize, Deserialize)]
struct OverlayState {
    pid: u32,
    name: String,
    #[serde(default)]
    view: OverlayView,
    executable: PathBuf,
}

#[derive(Default)]
struct OverlaySupervisor {
    child: Option<Child>,
    name: Option<String>,
    view: OverlayView,
}

static SUPERVISOR: OnceLock<Mutex<OverlaySupervisor>> = OnceLock::new();

pub fn handle(root: &Path, request: &OverlayRequest) -> Result<()> {
    let supervisor = SUPERVISOR.get_or_init(|| Mutex::new(OverlaySupervisor::default()));
    let mut supervisor = supervisor
        .lock()
        .map_err(|_| anyhow::anyhow!("overlay supervisor lock was poisoned"))?;
    supervisor.refresh();
    match request.operation {
        OverlayOperation::Show => {
            let name = request
                .name
                .as_deref()
                .context("overlay show requires a layout name")?;
            supervisor.show(root, name, request.view)
        }
        OverlayOperation::Hide => supervisor.hide(),
        OverlayOperation::Toggle => {
            let name = request
                .name
                .as_deref()
                .context("overlay toggle requires a layout name")?;
            if supervisor.name.as_deref() == Some(name) {
                supervisor.hide()
            } else {
                supervisor.show(root, name, request.view)
            }
        }
    }
}

pub fn active_name() -> Option<String> {
    let supervisor = SUPERVISOR.get_or_init(|| Mutex::new(OverlaySupervisor::default()));
    let mut supervisor = supervisor.lock().ok()?;
    supervisor.refresh();
    supervisor.active_label().or_else(read_live_state_name)
}

impl OverlaySupervisor {
    fn show(&mut self, root: &Path, name: &str, view: OverlayView) -> Result<()> {
        validate_layout_name(name)?;
        let path = root.join("layouts").join(format!("{name}.yml"));
        let path = if path.exists() {
            path
        } else {
            root.join("layouts").join(format!("{name}.yaml"))
        };
        Layout::load(&path)?;
        let executable = overlay_executable()?;
        if self.update_existing_view(name, view, &executable)? {
            return Ok(());
        }

        self.hide()?;
        let child = Command::new(&executable)
            .arg("overlay-host")
            .arg(&path)
            .args(["--view", view.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .context("could not start native overlay host")?;
        write_state(&OverlayState {
            pid: child.id(),
            name: name.to_owned(),
            view,
            executable,
        })?;
        self.name = Some(name.to_owned());
        self.view = view;
        self.child = Some(child);
        Ok(())
    }

    fn update_existing_view(
        &mut self,
        name: &str,
        view: OverlayView,
        executable: &Path,
    ) -> Result<bool> {
        let pid = if self.name.as_deref() == Some(name) {
            self.child.as_ref().map(Child::id)
        } else {
            None
        };
        let pid = pid.or_else(|| {
            read_state()
                .filter(|state| state.name == name && process_matches(state))
                .map(|state| state.pid)
        });
        let Some(pid) = pid else {
            return Ok(false);
        };

        send_view_signal(pid, view)?;
        write_state(&OverlayState {
            pid,
            name: name.to_owned(),
            view,
            executable: executable.to_path_buf(),
        })?;
        self.name = Some(name.to_owned());
        self.view = view;
        Ok(true)
    }

    fn hide(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
            let _ = child.wait();
        } else if let Some(state) = read_state()
            && process_matches(&state)
        {
            let _ = unsafe { libc::kill(state.pid as i32, libc::SIGTERM) };
        }
        self.name = None;
        self.view = OverlayView::Base;
        let _ = fs::remove_file(state_path());
        Ok(())
    }

    fn refresh(&mut self) {
        if self
            .child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            self.child = None;
            self.name = None;
            self.view = OverlayView::Base;
            let _ = fs::remove_file(state_path());
        }
    }

    fn active_label(&self) -> Option<String> {
        self.name
            .as_ref()
            .map(|name| overlay_label(name, self.view))
    }
}

fn validate_layout_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains("..")
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("invalid layout name '{name}'");
    }
    Ok(())
}

fn overlay_executable() -> Result<PathBuf> {
    let current = std::env::current_exe().context("could not locate current executable")?;
    if current.file_name().and_then(|name| name.to_str()) == Some("xretype") {
        Ok(current)
    } else {
        Ok(current.with_file_name("xretype"))
    }
}

fn state_path() -> PathBuf {
    socket_path()
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .join("overlay.json")
}

fn write_state(state: &OverlayState) -> Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    fs::write(&path, serde_json::to_vec(state)?)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn read_state() -> Option<OverlayState> {
    serde_json::from_slice(&fs::read(state_path()).ok()?).ok()
}

fn read_live_state_name() -> Option<String> {
    let state = read_state()?;
    if process_matches(&state) {
        Some(overlay_label(&state.name, state.view))
    } else {
        let _ = fs::remove_file(state_path());
        None
    }
}

fn overlay_label(name: &str, view: OverlayView) -> String {
    match view {
        OverlayView::Base => name.to_owned(),
        OverlayView::Prefix => format!("{name} (Space prefix)"),
    }
}

fn view_signal(view: OverlayView) -> i32 {
    match view {
        OverlayView::Base => libc::SIGUSR2,
        OverlayView::Prefix => libc::SIGUSR1,
    }
}

fn send_view_signal(pid: u32, view: OverlayView) -> Result<()> {
    let result = unsafe { libc::kill(pid as i32, view_signal(view)) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error()).context("could not update overlay view")
    }
}

fn process_matches(state: &OverlayState) -> bool {
    let process_executable = fs::read_link(format!("/proc/{}/exe", state.pid)).ok();
    let expected = fs::canonicalize(&state.executable).ok();
    process_executable.is_some() && process_executable == expected
}

struct HostStateGuard(u32);

impl Drop for HostStateGuard {
    fn drop(&mut self) {
        if read_state().is_some_and(|state| state.pid == self.0) {
            let _ = fs::remove_file(state_path());
        }
    }
}

pub fn run_host(layout_path: &Path, view: OverlayView) -> Result<()> {
    let layout = Layout::load(layout_path)?;
    let _state_guard = HostStateGuard(std::process::id());
    let application = Application::new(
        Some("io.xretype.overlay"),
        gio::ApplicationFlags::NON_UNIQUE,
    );
    application.connect_activate(move |application| build_window(application, &layout, view));
    let app_for_term = application.clone();
    glib::source::unix_signal_add_local(libc::SIGTERM, move || {
        app_for_term.quit();
        glib::ControlFlow::Break
    });
    let app_for_int = application.clone();
    glib::source::unix_signal_add_local(libc::SIGINT, move || {
        app_for_int.quit();
        glib::ControlFlow::Break
    });
    application.run_with_args(&["xretype-overlay"]);
    Ok(())
}

fn build_window(application: &Application, layout: &Layout, view: OverlayView) {
    install_css();
    let window = ApplicationWindow::builder()
        .application(application)
        .title("xretype-overlay")
        .decorated(false)
        .focusable(false)
        .build();
    window.init_layer_shell();
    window.set_namespace(Some("xretype-overlay"));
    window.set_layer(Layer::Top);
    window.set_anchor(Edge::Top, true);
    window.set_margin(Edge::Top, 6);
    window.set_exclusive_zone(0);
    window.set_keyboard_mode(KeyboardMode::None);
    window.add_css_class("overlay-window");
    window.connect_realize(|window| {
        if let Some(surface) = window.surface() {
            surface.set_input_region(&gtk4::cairo::Region::create());
        }
    });

    let center = CenterBox::new();
    center.set_center_widget(Some(&keyboard_card(layout, view)));
    window.set_child(Some(&center));

    let prefix_center = center.clone();
    let prefix_layout = layout.clone();
    glib::source::unix_signal_add_local(libc::SIGUSR1, move || {
        prefix_center.set_center_widget(Some(&keyboard_card(&prefix_layout, OverlayView::Prefix)));
        glib::ControlFlow::Continue
    });
    let base_center = center.clone();
    let base_layout = layout.clone();
    glib::source::unix_signal_add_local(libc::SIGUSR2, move || {
        base_center.set_center_widget(Some(&keyboard_card(&base_layout, OverlayView::Base)));
        glib::ControlFlow::Continue
    });
    window.present();
}

fn keyboard_card(layout: &Layout, view: OverlayView) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 6);
    card.add_css_class("keyboard-card");
    card.set_size_request(1120, -1);

    let title_text = match view {
        OverlayView::Base => layout.name.clone(),
        OverlayView::Prefix => format!("{} · {} prefix", layout.name, layout.prefix),
    };
    let title = Label::new(Some(&title_text));
    title.set_halign(Align::Start);
    title.add_css_class("overlay-title");
    card.append(&title);
    let hint_text = match view {
        OverlayView::Base => format!(
            "{}: alternate prefix  ·  Esc: exit  ·  gold = substituted symbol",
            layout.prefix
        ),
        OverlayView::Prefix => format!(
            "{} armed  ·  press a gold symbol  ·  {} {} = literal space  ·  Esc: cancel",
            layout.prefix, layout.prefix, layout.prefix
        ),
    };
    let hint = Label::new(Some(&hint_text));
    hint.set_halign(Align::Start);
    hint.add_css_class("overlay-hint");
    card.append(&hint);

    let substitutions = layout.substitutions();
    for row_specs in keyboard_rows() {
        let row = GtkBox::new(Orientation::Horizontal, 4);
        row.set_homogeneous(false);
        for spec in row_specs {
            let substitution = spec.gid.and_then(|group| substitutions.get(group));
            row.append(&keyboard_key(&spec, substitution, &layout.prefix, view));
        }
        card.append(&row);
    }
    card
}

#[derive(Clone, Copy)]
enum KeyKind {
    Dual,
    Alpha,
    Mod,
    Space,
}

#[derive(Clone, Copy)]
struct KeySpec {
    kind: KeyKind,
    gid: Option<&'static str>,
    top: &'static str,
    bottom: &'static str,
    width: i32,
}

fn keyboard_key(
    spec: &KeySpec,
    substitution: Option<&crate::xremap::KeySubstitution>,
    prefix_key: &str,
    view: OverlayView,
) -> Frame {
    let frame = Frame::new(None);
    frame.add_css_class(if matches!(spec.kind, KeyKind::Mod | KeyKind::Space) {
        "key-mod"
    } else {
        "key-standard"
    });
    let is_prefix_key = spec.gid == Some(prefix_key);
    if view == OverlayView::Prefix && is_prefix_key {
        frame.add_css_class("prefix-key-active");
    }
    if view == OverlayView::Prefix
        && !is_prefix_key
        && substitution.is_none_or(|value| value.prefix.is_empty())
    {
        frame.add_css_class("prefix-unavailable");
    }
    frame.set_size_request(spec.width, 48);
    frame.set_hexpand(matches!(spec.kind, KeyKind::Space));
    let contents = GtkBox::new(Orientation::Vertical, 0);
    contents.set_valign(Align::Center);
    let base = substitution.map(|value| value.base.as_str()).unwrap_or("");
    let prefix = substitution
        .map(|value| value.prefix.as_str())
        .unwrap_or("");
    match spec.kind {
        KeyKind::Mod => {
            let label = Label::new(Some(spec.bottom));
            label.add_css_class("key-mod-label");
            contents.append(&label);
        }
        KeyKind::Space => {
            let label = Label::new(Some(if !base.is_empty() {
                base
            } else if !prefix.is_empty() {
                prefix
            } else if is_prefix_key {
                prefix_key
            } else {
                " "
            }));
            if !base.is_empty() || !prefix.is_empty() || is_prefix_key {
                label.add_css_class("substitution");
            }
            contents.append(&label);
        }
        KeyKind::Alpha | KeyKind::Dual => {
            let top_text = if !prefix.is_empty() { prefix } else { spec.top };
            let bottom_text = if !base.is_empty() { base } else { spec.bottom };
            let top = Label::new(Some(top_text));
            top.add_css_class("key-top");
            if !prefix.is_empty() {
                top.add_css_class("substitution");
                if view == OverlayView::Prefix {
                    top.add_css_class("prefix-symbol-active");
                }
            }
            let bottom = Label::new(Some(bottom_text));
            bottom.add_css_class("key-bottom");
            if !base.is_empty() {
                bottom.add_css_class("substitution");
                if view == OverlayView::Prefix {
                    bottom.add_css_class("prefix-symbol-inactive");
                }
            }
            contents.append(&top);
            contents.append(&bottom);
        }
    }
    frame.set_child(Some(&contents));
    frame
}

fn dual(gid: &'static str, top: &'static str, bottom: &'static str) -> KeySpec {
    KeySpec {
        kind: KeyKind::Dual,
        gid: Some(gid),
        top,
        bottom,
        width: 68,
    }
}

fn alpha(letter: &'static str) -> KeySpec {
    KeySpec {
        kind: KeyKind::Alpha,
        gid: Some(letter),
        top: "",
        bottom: letter,
        width: 68,
    }
}

fn modifier(label: &'static str, width: i32) -> KeySpec {
    KeySpec {
        kind: KeyKind::Mod,
        gid: None,
        top: "",
        bottom: label,
        width,
    }
}

fn keyboard_rows() -> Vec<Vec<KeySpec>> {
    vec![
        vec![
            dual("Grave", "~", "`"),
            dual("1", "!", "1"),
            dual("2", "@", "2"),
            dual("3", "#", "3"),
            dual("4", "$", "4"),
            dual("5", "%", "5"),
            dual("6", "^", "6"),
            dual("7", "&", "7"),
            dual("8", "*", "8"),
            dual("9", "(", "9"),
            dual("0", ")", "0"),
            dual("Minus", "_", "-"),
            dual("Equal", "+", "="),
            modifier("Backspace", 132),
        ],
        vec![
            modifier("Tab", 98),
            alpha("q"),
            alpha("w"),
            alpha("e"),
            alpha("r"),
            alpha("t"),
            alpha("y"),
            alpha("u"),
            alpha("i"),
            alpha("o"),
            alpha("p"),
            dual("BracketLeft", "{", "["),
            dual("BracketRight", "}", "]"),
            dual("Backslash", "|", "\\"),
        ],
        vec![
            modifier("Caps", 116),
            alpha("a"),
            alpha("s"),
            alpha("d"),
            alpha("f"),
            alpha("g"),
            alpha("h"),
            alpha("j"),
            alpha("k"),
            alpha("l"),
            dual("Semicolon", ":", ";"),
            dual("Apostrophe", "\"", "'"),
            modifier("Enter", 146),
        ],
        vec![
            modifier("Shift", 154),
            alpha("z"),
            alpha("x"),
            alpha("c"),
            alpha("v"),
            alpha("b"),
            alpha("n"),
            alpha("m"),
            dual("Comma", "<", ","),
            dual("Dot", ">", "."),
            dual("Slash", "?", "/"),
            modifier("Shift", 154),
        ],
        vec![
            modifier("Ctrl", 82),
            modifier("Win", 82),
            modifier("Alt", 82),
            KeySpec {
                kind: KeyKind::Space,
                gid: Some("Space"),
                top: "",
                bottom: " ",
                width: 470,
            },
            modifier("Alt", 82),
            modifier("Win", 82),
            modifier("Menu", 82),
            modifier("Ctrl", 82),
        ],
    ]
}

fn install_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        r#"
        .overlay-window { background: transparent; }
        .keyboard-card {
            background: rgba(24, 24, 32, 0.70);
            border: 1px solid rgba(160, 160, 200, 0.47);
            border-radius: 12px;
            padding: 10px 14px;
        }
        .key-standard, .key-mod {
            border: 1px solid rgba(140, 140, 180, 0.40);
            border-radius: 6px;
            padding: 3px;
        }
        .key-standard { background: rgba(40, 40, 58, 0.82); }
        .key-mod { background: rgba(55, 55, 72, 0.88); }
        .prefix-key-active {
            background: rgba(126, 91, 22, 0.94);
            border-color: rgba(255, 212, 121, 0.95);
        }
        .prefix-unavailable { opacity: 0.42; }
        .overlay-title { color: white; font-size: 17px; font-weight: 600; }
        .overlay-hint { color: #a0a0b0; font-size: 11px; }
        .key-top { color: #c8c8d8; font-size: 12px; }
        .key-bottom { color: #eaeaea; font-size: 16px; font-weight: 700; }
        .key-mod-label { color: #c8c8d8; font-size: 11px; font-weight: 600; }
        .substitution { color: #ffd479; font-weight: 700; }
        .prefix-symbol-active { color: #fff0b8; font-size: 17px; }
        .prefix-symbol-inactive { color: #777785; }
        "#,
    );
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlayView, overlay_label, validate_layout_name, view_signal};

    #[test]
    fn layout_name_cannot_escape_layout_directory() {
        assert!(validate_layout_name("math").is_ok());
        assert!(validate_layout_name("../secret").is_err());
        assert!(validate_layout_name("a/b").is_err());
    }

    #[test]
    fn prefix_view_is_visible_in_active_overlay_status() {
        assert_eq!(
            overlay_label("logic", OverlayView::Prefix),
            "logic (Space prefix)"
        );
    }

    #[test]
    fn overlay_views_have_distinct_update_signals() {
        assert_eq!(view_signal(OverlayView::Prefix), libc::SIGUSR1);
        assert_eq!(view_signal(OverlayView::Base), libc::SIGUSR2);
    }
}
