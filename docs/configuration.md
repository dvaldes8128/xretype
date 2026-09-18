# Configuration

xretype reads TOML from `$XDG_CONFIG_HOME/xretype/config.toml`. When
`XDG_CONFIG_HOME` is unset, the path is `~/.config/xretype/config.toml`.
Missing default configuration is allowed and uses built-in defaults. A path
passed with `--config FILE` must exist and parse successfully.

The same file must be visible to `xretype` and `xretyped`. When the daemon uses
a non-default file, update its systemd `ExecStart` to include `--config FILE`
and pass the same global option to client-side commands such as `validate` and
`xremap generate`.

For daemon-backed actions, runtime settings such as the input backend and
default info file come from the daemon's configuration. Passing `--config` only
to the client does not reconfigure an already running daemon. The client still
uses its local configuration for error-notification policy and client-side
commands.

Unknown sections and keys are rejected so misspellings cannot silently change
behavior.

## Complete example

```toml
[info]
file = "/home/you/.config/personal_info.json"

[input]
backend = "libei"
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

TOML paths are literal paths. xretype does not expand `~`, `$HOME`, or other
shell syntax inside the file. Omit a path setting to use its default, or write
an absolute path.

## `info`

`info.file` points to the JSON document used by `xretype info` and workflow
`info` actions.

- Default: `$XDG_CONFIG_HOME/personal_info.json`
- The root may contain any valid JSON for direct CLI lookups.
- Generated xremap personal-data menus impose additional naming and value rules
  described in [xremap integration](xremap-integration.md#personal-information-menus).

Protect this file as user-only data:

```bash
chmod 600 ~/.config/personal_info.json
```

## `input`

`input.backend` accepts `libei` or `ydotool` and defaults to `libei`.

### libei

libei uses Enigo's Wayland RemoteDesktop portal path. It does not require a
privileged virtual-input daemon, but the compositor can ask the user to approve
the session. Keeping xretyped alive allows that input session to be reused.

### ydotool

ydotool launches the `ydotool` client for each input operation and relies on an
existing `ydotoold` service. It is often convenient for frequent hotkeys after
the host has already configured the daemon and its device permissions.

The ydotool backend supports the named keys in the CLI reference and unshifted
ASCII keys. Direct `type` actions are passed to `ydotool type`, which handles
the full text.

### Paste timing

`input.paste_delay_ms` is the delay between offering clipboard text and
injecting Ctrl+V. It defaults to `30` milliseconds.

`input.clipboard_serve_ms` is how long the temporary clipboard offer remains
available for the target application. It defaults to `500` milliseconds and
must be greater than `paste_delay_ms`.

Increase `paste_delay_ms` when a compositor or target application sometimes
pastes the previous clipboard value. Increase `clipboard_serve_ms` when an
application requests clipboard data unusually late. Longer values also keep a
sensitive offer available for longer.

After the serve window, xretype clears the clipboard only if it still contains
the value xretype offered. A value copied by the user in the meantime is left
untouched.

## `feedback`

`feedback.notify_errors` defaults to `true`. When enabled, failed client
commands make a best-effort desktop error notification in addition to writing
the error to stderr. Disable it for scripts that should report failures only
through their exit status and output:

```toml
[feedback]
notify_errors = false
```

## `automation`

`automation.file` points to the version 1 workflow document.

- Default: `$XDG_CONFIG_HOME/xretype/automations.yml`
- A missing file is treated as an empty workflow registry.
- xretyped watches the containing directory and reloads the target file after
  changes.
- An invalid replacement is reported in daemon status while the last valid
  registry remains active.

## `xremap`

`xremap.root` is the directory containing `config.yml`, `layouts/`, and
`templates/`. It defaults to `$XDG_CONFIG_HOME/xremap`.

`xremap.executable` is the exact xretype path embedded in generated xremap
`launch` arrays. When omitted, the generator searches `PATH`, then
`~/.cargo/bin/xretype`, then uses the running executable. Setting an absolute
path makes generation reproducible across interactive shells and systemd.

## Reload behavior

Changing `automations.yml` reloads workflows automatically. Changing
`config.toml` does not reconfigure a running daemon; restart it:

```bash
systemctl --user restart xretyped.service
```

Use `xretype daemon reload` for an immediate workflow reload and error result,
or `xretype daemon status` to inspect the most recent automatic reload.
