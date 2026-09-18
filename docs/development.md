# Development

## Repository layout

The crate builds a shared library plus two binaries:

```text
src/
├── main.rs                 xretype CLI entry point
├── bin/xretyped.rs         daemon entry point and Wayland startup recovery
├── cli.rs                  clap command definitions
├── config.rs               strict TOML configuration
├── actions/                primitive action model and executor
├── automation/             workflow parser, validator, binder, and runner
├── clipboard/              temporary sensitive/public clipboard offers
├── daemon/                 Unix-socket protocol, queue, watcher, and status
├── input/                  libei/Enigo and ydotool backends
├── sources/                literal, stdin, and JSON value resolution
├── visual/                 notifications and GTK layer-shell overlays
└── xremap/                 layout model and marked-block generator
```

The daemon uses one worker queue for actions and workflows. That serialization
prevents concurrent input sequences and clipboard offers from interleaving.
Socket connections are handled independently, while the worker owns the warm
input runtime and current workflow registry.

## Local checks

Run the standard checks from the repository root:

```bash
cargo fmt --all --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Exercise parser help after changing CLI definitions:

```bash
cargo run -- --help
cargo run -- paste --help
cargo run -- xremap generate --help
```

Validate the checked-in workflow example without depending on the user's
configuration:

```bash
example_root="$(mktemp -d)"
mkdir -p "$example_root/xretype"
cp examples/config.toml "$example_root/xretype/config.toml"
cp examples/automations.yml "$example_root/xretype/automations.yml"
XDG_CONFIG_HOME="$example_root" cargo run -- validate
```

Copy `examples/personal_info.json` and `examples/xremap/` to the corresponding
paths below the same temporary root to exercise layout loading and generation.

## Testing principles

Most logic is deliberately separated from desktop side effects:

- key parsing and ydotool event generation are pure functions;
- workflow parsing and static validation do not initialize input;
- marked-block replacement and layout rendering can be tested as strings;
- clipboard timing helpers can be tested with supplied readers;
- daemon watcher filters and runtime-directory permissions are unit-tested.

Desktop smoke tests should use dummy text, a disposable target, and an explicit
backend. Do not place secrets in test commands, fixtures, logs, or process
arguments.

## Generator compatibility

Generator changes should preserve text outside marked blocks and continue to
accept the two legacy Python begin markers. A normal write must remain atomic,
preserve the destination mode, and save the immediately previous configuration
as `config.yml.bak`.

When changing layout generation, verify base mappings, prefix mappings, literal
prefix insertion, modifier passthrough, Esc cleanup, overlay transitions, and
newline conversion. When changing personal-info generation, verify that output
contains only paths and never resolved values.

## IPC compatibility

The client and daemon exchange one newline-terminated JSON envelope over a
user-only Unix socket. Requests are limited to 1 MiB and include a protocol
version. An incompatible protocol must fail explicitly rather than being
interpreted as an older request.

Avoid adding sensitive values to status, debug formatting, or error context.
The request protocol necessarily carries action payloads locally, but normal
status and reload errors must remain metadata-only.

## Documentation updates

Behavior changes should update all three relevant surfaces:

1. clap help or schema validation in code;
2. the focused page under `docs/`;
3. a checked-in example when the file format changes.

Keep the README as an entry point rather than duplicating the full references.
All example personal information must be visibly fictitious.
