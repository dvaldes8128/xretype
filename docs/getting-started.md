# Getting started

This guide installs xretype from source, starts its per-user daemon, and runs a
small workflow. The checked-in [example set](../examples/README.md) provides all
of the files used below.

## 1. Install prerequisites

xretype is intended for a Linux desktop running Wayland. Building it requires:

- Rust 1.89 or newer
- a C compiler and `pkg-config`
- development libraries for libei, D-Bus, GTK4, and gtk4-layer-shell
- xremap when using the generated keyboard integration
- ydotool and a running `ydotoold` service only when choosing the ydotool input
  backend

Package names differ by distribution. A missing native package normally appears
as a `pkg-config` error during `cargo build`.

## 2. Build and install

From the repository root:

```bash
cargo test
cargo install --path . --force
```

Cargo installs both `xretype` and `xretyped` in `~/.cargo/bin`. Confirm that the
client is available:

```bash
xretype --version
xretype --help
```

If `xretype` is not found, add Cargo's binary directory to the login shell's
`PATH`.

## 3. Install a starter configuration

For a fresh setup, copy the non-secret examples:

```bash
mkdir -p ~/.config/xretype
cp examples/config.toml ~/.config/xretype/config.toml
cp examples/automations.yml ~/.config/xretype/automations.yml
```

The example configuration selects libei and otherwise uses xretype's normal
paths. To try personal-information lookup, inspect and replace every sample
value before installing the example:

```bash
cp examples/personal_info.json ~/.config/personal_info.json
chmod 600 ~/.config/personal_info.json
```

Never overwrite an existing personal-information file without backing it up.

Validate the workflow file without executing any actions:

```bash
xretype validate
```

Expected output:

```text
valid: 5 workflow(s)
```

## 4. Start the daemon

Install the systemd user unit:

```bash
mkdir -p ~/.config/systemd/user
cp contrib/systemd/xretyped.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now xretyped.service
```

Check both systemd and xretype's own status endpoint:

```bash
systemctl --user status xretyped.service
xretype daemon status
```

The status output includes the daemon version and PID, loaded workflow count,
configuration generation, queue depth, active workflow and overlay, and the
last reload error if one exists.

## 5. Exercise the safe commands

Notifications and validation do not inject keyboard input:

```bash
xretype notify --title "xretype setup" "daemon communication works"
xretype run announce --param message='workflow execution works'
```

Next, focus a disposable text field and run one of these commands from a
shortcut or a second terminal:

```bash
xretype type "typed directly"
xretype paste "pasted as sensitive text"
xretype paste --public "allowed in clipboard history"
xretype info Contact Email
```

`paste` and `info` use sensitive clipboard handling unless `--public` is
present. `type` bypasses the clipboard.

With libei, the desktop may display a RemoteDesktop portal prompt when the
input session is first established. With ydotool, `ydotoold` must already be
running and accessible to the user.

## 6. Connect xremap

The optional companion service in `contrib/systemd/xremap.service` shows the
required xremap flags: generated configurations use `launch` actions, so xremap
must start with `--allow-launch=true`.

Before installing that unit, edit its device selector and executable path for
the local machine. Then follow the complete [xremap integration
guide](xremap-integration.md) to install the layout sources, templates, and
generated-block markers.

## Next steps

- Choose and tune an input backend in [Configuration](configuration.md).
- Build real automations with the [Workflow language](workflows.md).
- Add symbol layers and personal-data menus with [xremap
  integration](xremap-integration.md).
- If setup fails, use the ordered checks in [Troubleshooting](troubleshooting.md).
