# xretype

`xretype` is the automation runtime for an
[xremap](https://github.com/xremap/xremap) keyboard setup on Linux/Wayland.
xremap recognizes devices, shortcuts, and modes; the persistent `xretyped`
service serializes typing, protected paste, personal-information lookup,
workflows, notifications, and native keyboard overlays.

## Features

- Type or paste literal text and stdin through libei or ydotool.
- Exclude sensitive paste values from compatible clipboard histories by default.
- Resolve nested scalar values from `~/.config/personal_info.json`.
- Run typed, parameterized, composable YAML workflows.
- Keep input sessions and workflow configuration alive in `xretyped`.
- Render non-focusable GTK4 layer-shell layout overlays on Wayland.
- Generate the personal-information and layout blocks in xremap `config.yml`.
- Validate, preview, and check generated configuration before writing it.

## Requirements and installation

The project requires Rust 1.89 or newer plus the native development packages
for Enigo/libei, arboard, D-Bus, GTK4, and gtk4-layer-shell.

```bash
cargo install --path . --force
mkdir -p ~/.config/systemd/user
cp contrib/systemd/xretyped.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now xretyped.service
```

The companion xremap unit in `contrib/systemd/xremap.service` starts after
`xretyped`, explicitly enables launches, and watches `config.yml` for changes.

## Runtime commands

Commands use `xretyped` by default. Add global `--standalone` to execute a
primitive or workflow in the caller for recovery.

```bash
xretype type "hello world"
printf 'hello world' | xretype type --stdin

# Sensitive by default; --public permits normal clipboard history.
xretype paste "secret or long text"
xretype paste --public "ordinary text"

xretype info identity email
xretype info identity.email
xretype info --type identity email

xretype key enter
xretype combo ctrl shift t
xretype sleep 100
xretype notify --title xremap "Developer layer enabled"

xretype overlay show math
xretype overlay hide
xretype overlay toggle logic

xretype daemon status
xretype daemon reload
```

If the daemon is unavailable, commands fail with its socket path and suggest
`--standalone`; fallback is never silent.

## Automation language

The default workflow file is `~/.config/xretype/automations.yml`:

```yaml
version: 1

workflows:
  delayed-paste:
    parameters:
      text: { type: string }
      delay-ms: { type: integer, default: 30 }
      public: { type: boolean, default: false }
    actions:
      - action: sleep
        milliseconds: { param: delay-ms }
      - action: paste
        text: { param: text }
        public: { param: public }

  announce-and-paste:
    parameters:
      text: { type: string }
    actions:
      - action: notify
        title: xretype
        body: Running delayed paste
      - action: run
        workflow: delayed-paste
        with:
          text: { param: text }
```

Run and validate it with:

```bash
xretype validate
xretype run delayed-paste --param text='hello' --param delay-ms=50
```

Parameter types are `string`, `integer`, and `boolean`. Values are literals or
explicit `{ param: name }` references. Supported actions are `type`, `paste`,
`info`, `key`, `combo`, `sleep`, `notify`, `overlay`, and `run`. Workflows are
sequential and stop at the first failure. Validation rejects unknown fields,
bad types, missing arguments, and static call cycles before execution.

The daemon watches the file, debounces edits, and retains the last valid
registry when a reload fails. `xretype daemon status` reports the error without
logging parameter or text values.

## xremap generation

The generator keeps source assets under `~/.config/xremap`:

- `layouts/*.yml` describes symbol/snippet layers.
- `templates/*.tmpl` controls the generated block arrangement.
- `config.yml` contains the marked output blocks.
- `~/.config/personal_info.json` supplies only the personal-data hierarchy;
  values are resolved at execution time and are not copied into `config.yml`.

```bash
xretype xremap layouts
xretype xremap generate --dry-run
xretype xremap generate --check
xretype xremap generate
xretype xremap generate --only layouts
xretype xremap generate --only personal-info
```

Generation recognizes the legacy Python markers, replaces only marked text,
writes atomically, and saves the immediately previous file as `config.yml.bak`.
The native output preserves the former generators' selector, nested-mode,
notification, newline, and Alt-passthrough behavior. Generated overlay actions
use `xretype overlay`; process-wide `pkill` is no longer needed.

## Configuration

Settings live at `$XDG_CONFIG_HOME/xretype/config.toml` or
`~/.config/xretype/config.toml`:

```toml
[info]
file = "/home/you/.config/personal_info.json"

[input]
backend = "ydotool" # or "libei"
paste_delay_ms = 30
clipboard_serve_ms = 500

[feedback]
notify_errors = true

[automation]
file = "/home/you/.config/xretype/automations.yml"

[xremap]
root = "/home/you/.config/xremap"
executable = "/home/you/.cargo/bin/xretype"
```

The `libei` backend follows the Wayland RemoteDesktop portal model. `ydotool`
uses an existing `ydotoold` service and is often preferable for frequent
one-shot hotkeys. Clipboard ownership remains inside xretype for both backends.

## Overlay behavior

The overlay uses GTK4 layer-shell on the top layer, does not reserve screen
space, requests no keyboard interactivity, and installs an empty pointer input
region. It displays a full ANSI keyboard with base/Shift substitutions merged
from the same typed layout model used by generation. A supervised child process
isolates GTK from the daemon; only that child is stopped on hide or replacement.

## Security and troubleshooting

- The daemon socket and overlay state live under `$XDG_RUNTIME_DIR/xretype`
  with user-only permissions.
- Sensitive paste excludes compatible clipboard history, but it is not a
  boundary against a compromised desktop session or unsupported clipboard
  manager.
- Workflow errors and status never include resolved text or parameter values.
- `--param NAME=VALUE` is visible briefly in process arguments; use `info`
  actions for stored personal data instead of passing secrets as parameters.
- Use `journalctl --user -u xretyped.service` for service failures,
  `xretype validate` for YAML problems, and `xretype --standalone ...` to
  distinguish daemon/IPC issues from input backend issues.
- At login, `xretyped` discovers the compositor socket under
  `$XDG_RUNTIME_DIR` when the desktop has not yet imported `WAYLAND_DISPLAY`
  into the systemd user manager.

The version-1 language intentionally omits branches, loops, retries, parallel
execution, string interpolation, and arbitrary shell commands. Those belong in
a later schema version once real workflows establish their required semantics.
