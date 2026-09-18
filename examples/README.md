# Examples

This directory is a complete, mutually compatible xretype example. Every value
in `personal_info.json` is fictitious.

```text
examples/
├── config.toml
├── automations.yml
├── personal_info.json
└── xremap/
    ├── config.yml
    ├── layouts/logic.yml
    └── templates/
        ├── xremap_layouts_block.tmpl
        └── xremap_personal_info_block.tmpl
```

## Install into a fresh account

Review the files before copying them. These commands are only for paths that do
not already contain a configuration you need to preserve.

```bash
mkdir -p ~/.config/xretype
cp examples/config.toml ~/.config/xretype/config.toml
cp examples/automations.yml ~/.config/xretype/automations.yml

cp examples/personal_info.json ~/.config/personal_info.json
chmod 600 ~/.config/personal_info.json

mkdir -p ~/.config/xremap/layouts ~/.config/xremap/templates
cp examples/xremap/config.yml ~/.config/xremap/config.yml
cp examples/xremap/layouts/logic.yml ~/.config/xremap/layouts/
cp examples/xremap/templates/*.tmpl ~/.config/xremap/templates/
```

Then validate and generate:

```bash
xretype validate
xretype xremap layouts
xretype xremap generate --dry-run
xretype xremap generate
```

The checked-in `xremap/config.yml` contains empty generated regions. Running the
generator fills them and creates `config.yml.bak` with the original skeleton.

## Try the workflows

The five examples demonstrate parameters, defaults, notifications, protected
paste, composition, JSON lookup, overlays, and sleep:

```bash
xretype run announce --param message='hello from a workflow'
xretype run delayed-paste --param text='sensitive by default'
xretype run delayed-paste --param text='public value' --param public=true
xretype run announce-and-run --param text='composed workflow'
xretype run paste-contact-email
xretype run show-logic-overlay
```

Run text-producing workflows only while a safe target field is focused. Replace
the sample personal information before using it as a real shortcut source.

## Adapt an existing xremap setup

Do not copy the skeleton over an existing `config.yml`. Instead:

1. Copy the two `.tmpl` files into its `templates/` directory.
2. Copy or adapt `layouts/logic.yml` under `layouts/`.
3. Add both marker pairs from the example to the existing top-level `keymap`
   sequence.
4. Preview with `xretype xremap generate --dry-run`.
5. Generate only after reviewing the complete output.

See [xremap integration](../docs/xremap-integration.md) for every validation and
generation rule.
