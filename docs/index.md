# xretype

xretype is a Linux/Wayland automation runtime for
[xremap](https://github.com/xremap/xremap). xremap recognizes devices,
shortcuts, and modes; xretype handles serialized input, protected paste,
personal-information lookup, workflows, notifications, and keyboard-layout
overlays.

The package contains two binaries:

- `xretype` is the command-line client and standalone recovery tool.
- `xretyped` is the persistent per-user service used by normal commands.

## Start here

New installations should begin with [Getting started](getting-started.md), then
use the focused guides as needed:

- [Configuration](configuration.md) documents every setting and input backend.
- [CLI reference](cli-reference.md) covers commands, options, and key names.
- [Workflows](workflows.md) defines the complete version 1 YAML language.
- [xremap integration](xremap-integration.md) covers generated menus, symbol
  layouts, templates, and overlays.
- [Troubleshooting](troubleshooting.md) provides ordered diagnostic checks.
- [Development](development.md) explains the architecture and project checks.

Complete, mutually compatible source files live in the repository's
[examples directory](https://github.com/dvaldes8128/xretype/tree/main/examples).

## Core behavior

- Type directly or paste through libei or ydotool.
- Exclude sensitive paste values from compatible clipboard histories by
  default, then clear xretype's temporary value when it remains on the
  clipboard.
- Resolve personal values from local JSON without copying them into generated
  xremap configuration.
- Run typed, composable YAML workflows through a persistent, serialized daemon.
- Generate xremap personal-data menus and symbol layouts from small source
  files.
- Display native GTK4 layer-shell keyboard overlays without taking focus.

## Safety and scope

Sensitive paste protects against accidental clipboard-history retention. It is
not a boundary against a compromised desktop session or a clipboard manager
that ignores exclusion requests.

Workflow status and errors do not include resolved text or parameter values.
Command-line parameters can still be visible briefly in process listings, so
stored secrets should be resolved with `info` actions.

Version 1 workflows are deterministic and sequential. They intentionally omit
branches, loops, retries, concurrency, interpolation, and arbitrary shell
commands.

The project source is available on
[GitHub](https://github.com/dvaldes8128/xretype) under the
[MIT License](https://github.com/dvaldes8128/xretype/blob/main/LICENSE).
