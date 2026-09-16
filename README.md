# xretype

`xretype` is a small Rust CLI for keyboard and typing automations launched by
[xremap](https://github.com/xremap/xremap). It keeps xremap responsible for
recognizing keys while giving text injection, protected paste, structured data,
and visual feedback one focused home.

This repository is an intentionally small v0.1 foundation. There is no daemon,
background service, or automation language yet.

## What works in v0.1

- Type literal text or stdin through Enigo's libei backend.
- Paste literal text or stdin through `arboard`, then inject `Ctrl+V`.
- Exclude pasted values from compatible clipboard histories by default.
- Resolve nested values from `~/.config/personal_info.json` with `info`.
- Send individual keys and key combinations.
- Sleep for a specified number of milliseconds.
- Show desktop notifications over the freedesktop notification interface.
- Parse overlay show, hide, and toggle commands through a placeholder backend.
- Represent future sequences without running them yet.

## Install

You need Rust 1.89 or newer and the native development packages required by
Enigo/libei, `arboard`, and D-Bus on your Linux distribution.

```bash
cargo install --path .
```

Enigo's libei support follows the Wayland security model and may require a
desktop permission prompt. Clipboard support requires a compositor implementing
a compatible data-control protocol.

## Commands

```bash
# Direct text injection; does not touch the clipboard.
xretype type "hello world"
printf 'hello world' | xretype type --stdin

# Sensitive by default: omitted from compatible clipboard histories.
xretype paste "secret or long text"
printf 'secret' | xretype paste --stdin

# Explicitly allow a normal clipboard-history entry.
xretype paste --public "ordinary text"

# Replacement for inject_info.sh. Both path forms are accepted.
xretype info identity email
xretype info identity.email
xretype info --file /path/to/data.json address home city
xretype info --type identity email

xretype key enter
xretype combo ctrl shift t
xretype sleep 100
xretype notify --title xremap "Developer layer enabled"

# Parsed now, intentionally not rendered in v0.1.
xretype overlay show navigation
```

The `info` command reads a scalar JSON value and pastes it with sensitive
history exclusion. `--type` bypasses the clipboard; `--public` opts into normal
clipboard history.

Example personal information file:

```json
{
  "identity": {
    "email": "person@example.com"
  },
  "address": {
    "home": {
      "city": "Example City"
    }
  }
}
```

An xremap mapping can stay simple:

```yaml
keymap:
  - name: Personal information
    remap:
      Super-Alt-E:
        launch: ["xretype", "info", "identity", "email"]
```

## Configuration

The optional config file lives at `$XDG_CONFIG_HOME/xretype/config.toml`, or
`~/.config/xretype/config.toml` when `XDG_CONFIG_HOME` is unset. Pass a different
file with the global `--config` option.

```toml
[info]
file = "/home/you/.config/personal_info.json"

[input]
paste_delay_ms = 30
clipboard_serve_ms = 500

[feedback]
notify_errors = true
```

`paste_delay_ms` gives the clipboard offer time to become available before
`Ctrl+V`. `clipboard_serve_ms` keeps this short-lived process available long
enough to serve the Wayland clipboard request.

## Sensitive paste model

Sensitive paste is the default. On Linux, `arboard` adds the KDE-compatible
`x-kde-passwordManagerHint` MIME marker so supporting clipboard managers do not
store the value in history. The text is still available to the focused
application for pasting.

This is history exclusion, not a general secrecy boundary. A malicious process,
an unsupported clipboard manager, or a compromised desktop session may still
read clipboard data. Use `--public` only when leaving the value in ordinary
clipboard history is acceptable.

## Architecture

```text
xremap -> xretype CLI -> action runtime
                         |-- sources: literal / stdin / JSON
                         |-- input: Enigo / libei
                         |-- clipboard: arboard / sensitive history exclusion
                         |-- visual: notifications / overlay placeholder
                         `-- automation: future sequence definitions
```

The core is a library crate. `main.rs` only parses arguments and reports errors;
the modules under `src/` are designed to remain independently testable as the
project grows.

## Deliberately deferred

- Overlay window rendering and themes.
- Loading and running named action sequences.
- A persistent `xretyped` daemon.
- Alternate input backends or automatic backend selection.
