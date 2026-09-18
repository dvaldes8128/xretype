# xretype

`xretype` is a Linux/Wayland automation runtime for
[xremap](https://github.com/xremap/xremap). xremap recognizes devices,
shortcuts, and modes; xretype handles the stateful work behind them: serialized
input, protected paste, personal-information lookup, workflows, notifications,
and keyboard-layout overlays.

The package installs two binaries:

- `xretype` is the command-line client and standalone recovery tool.
- `xretyped` is the persistent per-user service used by normal commands.

## Highlights

- Type text or paste it through libei or ydotool.
- Keep sensitive paste values out of compatible clipboard histories by default,
  then clear xretype's temporary clipboard value after the paste.
- Resolve values from a local JSON file without copying them into xremap's
  configuration.
- Define typed, composable YAML workflows with load-time validation.
- Generate xremap personal-data menus and symbol layouts from small source
  files.
- Show native, non-focusable GTK4 layer-shell keyboard overlays.
- Keep input sessions and validated workflows warm in a daemon that serializes
  requests.

## Quick start

xretype requires Linux on Wayland, Rust 1.89 or newer, and the native
development libraries used by libei, D-Bus, GTK4, and gtk4-layer-shell. For a
fresh setup:

```bash
cargo install --path . --force

mkdir -p ~/.config/xretype ~/.config/systemd/user
cp -n examples/config.toml ~/.config/xretype/config.toml
cp -n examples/automations.yml ~/.config/xretype/automations.yml
cp -n examples/personal_info.json ~/.config/personal_info.json
chmod 600 ~/.config/personal_info.json
cp -n contrib/systemd/xretyped.service ~/.config/systemd/user/

systemctl --user daemon-reload
systemctl --user enable --now xretyped.service

xretype daemon status
xretype validate
xretype notify "xretype is ready"
```

The copied personal-information values are fictitious; replace them before
creating real shortcuts.

Commands contact `xretyped` by default. Use the global `--standalone` option
when recovering from a daemon problem:

```bash
xretype type "hello world"
printf 'hello from stdin\n' | xretype paste --stdin
xretype info Contact Email
xretype run delayed-paste --param text='hello' --param delay-ms=50

xretype --standalone notify "running without the daemon"
```

If the daemon is unavailable, xretype reports the socket path and suggests
`--standalone`; it never falls back silently.

## Documentation

- [Getting started](docs/getting-started.md) — prerequisites, installation, and
  a first end-to-end setup
- [Configuration](docs/configuration.md) — every TOML setting and backend
  choice
- [CLI reference](docs/cli-reference.md) — commands, options, and accepted key
  names
- [Workflows](docs/workflows.md) — the version 1 YAML schema and every action
- [xremap integration](docs/xremap-integration.md) — layouts, personal-data
  menus, templates, generated blocks, and overlays
- [Troubleshooting](docs/troubleshooting.md) — daemon, Wayland, clipboard,
  workflow, and generator diagnostics
- [Development](docs/development.md) — architecture, checks, and contribution
  notes
- [Examples](examples/README.md) — a complete configuration that can be copied
  into a fresh account

## Safety model

Sensitive paste is the default. It asks compatible clipboard managers not to
retain the value and clears the clipboard afterward if the value is still
there. This protects against accidental history retention; it is not a
security boundary against a compromised desktop session or a clipboard manager
that ignores the exclusion request.

Workflow errors and daemon status do not include resolved text or parameter
values. Command-line `--param NAME=VALUE` arguments can still be briefly visible
to local process inspection, so stored secrets should be read with `info`
actions instead.

## Scope

Version 1 intentionally keeps workflows deterministic and sequential. It does
not provide branches, loops, retries, parallel execution, interpolation, or
arbitrary shell commands.

Licensed under the [MIT License](LICENSE).
