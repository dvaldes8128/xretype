# xremap integration

xretype turns small layout and personal-information sources into marked blocks
inside xremap's `config.yml`. It only replaces those blocks; configuration
outside the markers is preserved.

Generated actions use xremap's `launch` feature, so xremap must run with
`--allow-launch=true`.

## Source tree

The default root is `~/.config/xremap`:

```text
~/.config/xremap/
├── config.yml
├── layouts/
│   └── logic.yml
└── templates/
    ├── xremap_layouts_block.tmpl
    └── xremap_personal_info_block.tmpl
```

A complete source tree is available under `examples/xremap/`.

Configure a different root or embedded executable path in `config.toml`:

```toml
[xremap]
root = "/home/you/.config/xremap"
executable = "/home/you/.cargo/bin/xretype"
```

## Generated-block markers

`config.yml` must contain the exact marker pairs inside its top-level `keymap`
sequence:

```yaml
keymap:
  # BEGIN GENERATED PERSONAL_INFO (run: xretype xremap generate --only personal-info)
  # generated content appears here
  # END GENERATED PERSONAL_INFO

  # BEGIN GENERATED LAYOUTS (run: xretype xremap generate --only layouts)
  # generated content appears here
  # END GENERATED LAYOUTS
```

Legacy begin markers naming the former Python generator scripts are accepted
and replaced with the native markers on the next generation. End markers are
`# END GENERATED PERSONAL_INFO` and `# END GENERATED LAYOUTS`.

## Templates

The layout template must contain these literal placeholders:

```text
${trigger_block}
${menu_block}
${layers_block}
```

The personal-information template must contain:

```text
${trigger_block}
${layer0_block}
${layers_block}
```

The minimal templates in `examples/xremap/templates/` place a blank line between
sections. Placeholders are replaced textually, so surrounding comments or
additional static keymaps are allowed. Generated blocks already use the
indentation required for items under `keymap:`.

## Symbol layouts

Each `.yml` or `.yaml` file in `layouts/` defines one menu-selectable mode and
one overlay. Its filename is the layout ID used by overlay commands.

```yaml
name: Logic
selector: L
mode: logic_layer
prefix: Space
columns: 3

entries:
  - { key: a, symbol: "∧" }
  - { key: o, symbol: "∨" }
  - { key: Space-a, symbol: "∀" }
  - { key: Space-e, symbol: "∃" }
```

Fields behave as follows:

- `name` is the displayed layout name and must not be empty.
- `selector` is the single key chosen from the shared layouts menu. Selectors
  must be unique across files.
- `mode` is optional. Its default is a lowercase, underscore-separated slug of
  the name followed by `_layer`. It must be unique and cannot be `layouts`.
- `prefix` is optional and defaults to `Space`. It must be one non-combination
  key and cannot contain `-`.
- `columns` is optional, defaults to `3`, and must be positive. It is retained
  as layout metadata; the current native overlay is arranged as an ANSI
  keyboard.
- `entries` is a non-empty sequence of unique xremap key names and output
  strings. `label` is accepted as optional metadata.

An ordinary entry such as `a` applies in the base layer. An entry whose key is
`PREFIX-target`, such as `Space-a`, applies after the prefix is armed. The same
target may have both base and prefixed output. `Space-Space` is reserved: the
generated prefix layer uses it to return to the base layer and insert a literal
space.

Write the two characters `\\n` in a symbol to request an injected newline. The
generator converts that escape for the launch argument, and the overlay renders
it as `↵`.

The generator also emits:

- Esc mappings that leave the layout and hide its overlay
- Alt passthrough for base-layer entries
- Alt/Ctrl/Shift/Super passthrough for the prefix key
- overlay transitions between base and prefix views
- a paste action that returns the overlay to base after a prefixed symbol

Inspect layouts before generation:

```bash
xretype xremap layouts
```

This validates every file plus cross-file selector and mode uniqueness.

## Personal-information menus

The generator reads the same JSON document as `xretype info`. Objects become
nested modes and string leaves become paste actions. Values themselves are
never copied into `config.yml`; generated actions contain only JSON paths and
resolve the current value at execution time.

```json
{
  "Contact": {
    "Email": "person@example.com",
    "Phone": "+1 555 0100"
  },
  "Work": {
    "Company": "Example Company",
    "Address": {
      "Street": "100 Example Avenue",
      "City": "Example City"
    }
  }
}
```

Generated menu keys follow a deliberately explicit naming convention:

1. The key is the first ASCII digit in a name, if present.
2. Otherwise it is the first ASCII uppercase letter.
3. A name with neither cannot produce a generated selector.
4. Keys must be unique among siblings. Top-level category keys must also be
   unique.

For example, `Email` maps to `E`, `Phone` maps to `P`, and `2FA code` maps to
`2`. Names such as `email` fail because they have no uppercase letter or digit.
Siblings `Full name` and `Family name` conflict because both map to `F`.

Only top-level objects become categories, and only string leaves generate paste
actions. Non-empty nested objects become submenus; other nested JSON types do
not produce mappings. Avoid empty top-level category objects because they can
create a selector with no corresponding actions. Direct `xretype info` commands
can still resolve number and boolean scalar values.

Generated personal-data navigation begins with the F23 double-tap trigger.
Generated layout navigation begins with the F22 double-tap trigger. Those
triggers enter their respective selection modes and display notifications with
available choices.

Protect the source JSON with mode `0600`. Generated actions paste sensitively by
default, but the [safety model](../README.md#safety-model) still applies.

## Generate, inspect, and check

Preview the complete prospective configuration without writing:

```bash
xretype xremap generate --dry-run
```

Generate both blocks:

```bash
xretype xremap generate
```

Generate one block:

```bash
xretype xremap generate --only layouts
xretype xremap generate --only personal-info
```

Check for stale generated output in a verification script:

```bash
xretype xremap generate --check
```

Normal generation reads and validates every required source before replacing
`config.yml` atomically. If content changes, the immediately previous file is
written to `config.yml.bak` first. An unchanged run does not rewrite either
file.

`--root DIR` overrides the configured xremap root for `layouts` and `generate`.

## Install the complete example

On a new setup with no existing xremap configuration:

```bash
mkdir -p ~/.config/xremap/layouts ~/.config/xremap/templates
cp examples/xremap/config.yml ~/.config/xremap/config.yml
cp examples/xremap/layouts/logic.yml ~/.config/xremap/layouts/
cp examples/xremap/templates/*.tmpl ~/.config/xremap/templates/
xretype xremap generate
```

Do not replace an existing `config.yml`; instead add the two marker pairs to its
`keymap` sequence and copy only the source files you want.

The companion `contrib/systemd/xremap.service` is an example, not a portable
drop-in. Adjust its xremap executable and `--device` selector for the host before
enabling it.
