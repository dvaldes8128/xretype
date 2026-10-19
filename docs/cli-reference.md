# CLI reference

The general form is:

```text
xretype [--config FILE] [--standalone] COMMAND ...
```

`--config FILE` selects a TOML configuration. `--standalone` executes primitive
actions and workflows in the client instead of contacting xretyped. It is not
accepted for daemon operations and has no meaning for xremap generation.

All successful action commands are quiet. Validation, daemon inspection,
layout listing, and normal generation print a result.

## Text and clipboard commands

Type literal text directly through the selected input backend:

```bash
xretype type "hello world"
printf 'hello from stdin\n' | xretype type --stdin
```

Paste through a temporary clipboard offer and injected Ctrl+V:

```bash
xretype paste "sensitive by default"
xretype paste --public "ordinary clipboard text"
printf 'multi-line\ntext\n' | xretype paste --stdin
```

The trailing CR/LF sequence from stdin is removed. Other content, including
embedded newlines, is preserved.

`paste --overlay-reset NAME TEXT` returns the named overlay to its base view
before pasting. Generated prefix layouts use this internally.

## Personal information

Resolve a nested scalar from the configured JSON file and paste it:

```bash
xretype info Contact Email
xretype info Contact.Email
xretype info Work Address City
```

Dotted and separate path segments can be mixed. String, number, and boolean
values are converted to text. Null, object, array, and missing values are
errors.

Options:

- `--file FILE` overrides the configured JSON file for this action.
- `--type` types the value directly instead of using the clipboard.
- `--public` permits clipboard-history retention and conflicts with `--type`.

## Keys and combinations

Click one key or press a combination in the listed order:

```bash
xretype key enter
xretype key é
xretype combo ctrl shift t
xretype combo super l
```

The final combo item is clicked while preceding items are held, then held keys
are released in reverse order.

Named keys and aliases are:

- modifiers: `alt`/`option`, `ctrl`/`control`, `shift`, and
  `meta`/`super`/`win`/`windows`
- editing: `backspace`, `delete`/`del`, `insert`/`ins`, `enter`/`return`,
  `tab`, `space`, and `esc`/`escape`
- navigation: `up`, `down`, `left`, `right` and their `*arrow` forms; `home`,
  `end`, `pageup`/`pgup`, and `pagedown`/`pgdown`/`pgdn`
- system: `capslock`, `volumeup`, `volumedown`, and
  `volumemute`/`mute`
- function keys: `f1` through `f24`
- one Unicode character when using libei; the ydotool key/combo implementation
  accepts unshifted ASCII characters

Names are case-insensitive, and `_` or `-` inside a named key is ignored.

## Timing and notifications

```bash
xretype sleep 250
xretype notify "Build complete"
xretype notify --title "xremap" --timeout-ms 5000 "Layout enabled"
```

Sleep durations and notification timeouts are milliseconds. Notification
defaults are title `xretype` and timeout `1500`.

## Overlays

```bash
xretype overlay show logic
xretype overlay show logic --view prefix
xretype overlay toggle logic
xretype overlay hide
xretype overlay hide logic
```

The name is the layout filename without `.yml` or `.yaml`. `hide` accepts an
optional name for ergonomic use from xremap, but currently hides the supervised
xretype overlay regardless of that value.

Only one overlay is active. Showing another replaces it. Overlay windows do not
take focus, keyboard input, pointer input, or reserved screen space.

## Workflows

```bash
xretype validate
xretype run delayed-paste --param text='hello' --param delay-ms=50
xretype run delayed-paste --param text='hello' --param public=true
```

Repeat `--param NAME=VALUE` for multiple bindings. Duplicate and unknown names
are errors. Integer and boolean strings are parsed according to the workflow's
declared parameter type. Boolean true values are `true`, `1`, `yes`, and `on`;
false values are `false`, `0`, `no`, and `off`, ignoring case and surrounding
whitespace.

See [Workflows](workflows.md) for the complete schema.

## Machine-readable schemas

```bash
xretype schema automations
xretype schema config
xretype schema --write
```

`schema` prints the JSON Schema (draft 2020-12) for one configuration file.
`automations` describes the version 1 workflow document and `config` describes
`config.toml`. Redirect the output to a file and point an editor at it:

```bash
xretype schema automations > ~/.config/xretype/automations.schema.json
```

```yaml
# yaml-language-server: $schema=./automations.schema.json
version: 1
workflows: {}
```

`--write` refreshes every file committed under `schemas/` instead of printing;
it is what `just schema-generate` runs. Schemas are derived from the same Rust
types the loader parses, so they describe structure only: call cycles,
parameter type matching, `run.with` argument types, and nesting depth are
rejected by `xretype validate`, which remains authoritative.

## Daemon management

```bash
xretype daemon status
xretype daemon reload
```

`status` displays runtime state without exposing action text or parameter
values. `reload` immediately parses the configured workflow file and returns an
error without replacing the current registry if parsing or validation fails.

## Diagnostics

```bash
xretype doctor
```

`doctor` reads the session and the daemon, then prints one row per check:

```text
ok    session            wayland (wayland-0)
ok    runtime directory  /run/user/1000
ok    config             parsed
ok    automations        2 workflow(s)
ok    daemon socket      /run/user/1000/xretype/xretyped.sock
ok    daemon             responding (pid 3633)
ok    daemon version     0.1.0
ok    last reload error  none

8 ok, 0 warn, 0 fail
```

Each row is `ok`, `warn`, or `fail`. It is read-only, and like `schema` it runs
before the configuration is loaded, so it still reports when `config.toml` will
not parse. Only a `fail` row makes the command exit non-zero: a `warn` is
something degraded that still works, such as `XDG_RUNTIME_DIR` being unset and
falling back to `/tmp`.

Checks that need a daemon response — version match and last reload error — are
skipped when the daemon cannot be reached, rather than guessing. Reported
details never include resolved action text or parameter values.

## xremap generation

```bash
xretype xremap layouts
xretype xremap layouts --root /path/to/xremap

xretype xremap generate --dry-run
xretype xremap generate --check
xretype xremap generate
xretype xremap generate --only layouts
xretype xremap generate --only personal-info
```

`--dry-run` prints the entire prospective `config.yml`. `--check` exits with an
error when generated content differs. They conflict with each other. Normal
generation prints `updated` or `unchanged` and creates `config.yml.bak` only
when content changes.

See [xremap integration](xremap-integration.md) for the required source tree and
markers.

## Discovering current syntax

The CLI help is generated from the same definitions used by the parser:

```bash
xretype --help
xretype paste --help
xretype overlay show --help
xretype xremap generate --help
```
