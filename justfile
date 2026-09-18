set dotenv-load
set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]
set tempdir := "/tmp"

# List the available project workflows.
default:
    @just --list

# Build the debug binaries.
build:
    cargo build

# Build optimized release binaries.
build-release:
    cargo build --release

# Format Rust sources in place.
format:
    cargo fmt --all

# Verify Rust formatting without changing files.
format-check:
    cargo fmt --all --check

# Run all Rust tests and documentation tests.
test:
    cargo test

# Run Clippy and fail on every warning.
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Build the mdBook documentation.
docs-build:
    mdbook build

# Test code examples in the mdBook documentation.
docs-test:
    mdbook test

# Check and build the documentation exactly as CI expects.
docs-check: docs-test docs-build

# Serve the documentation locally with live reload.
docs-serve host="127.0.0.1" port="3000":
    mdbook serve --hostname "{{ host }}" --port "{{ port }}"

# Trigger the GitHub Pages workflow for the committed main branch.
docs-deploy: docs-check
    gh workflow run docs.yml --ref main
    @echo "Deployment requested: https://github.com/dvaldes8128/xretype/actions/workflows/docs.yml"

# Validate the checked-in workflow and xremap examples in isolation.
examples-check:
    #!/usr/bin/env bash
    set -euo pipefail
    fixture_root="$(mktemp -d /tmp/xretype-examples-XXXXXX)"
    trap 'rm -r -- "$fixture_root"' EXIT
    mkdir -p "$fixture_root/xretype" "$fixture_root/xremap/layouts" "$fixture_root/xremap/templates"
    cp examples/config.toml "$fixture_root/xretype/config.toml"
    cp examples/automations.yml "$fixture_root/xretype/automations.yml"
    cp examples/personal_info.json "$fixture_root/personal_info.json"
    cp examples/xremap/config.yml "$fixture_root/xremap/config.yml"
    cp examples/xremap/layouts/logic.yml "$fixture_root/xremap/layouts/logic.yml"
    cp examples/xremap/templates/xremap_layouts_block.tmpl "$fixture_root/xremap/templates/xremap_layouts_block.tmpl"
    cp examples/xremap/templates/xremap_personal_info_block.tmpl "$fixture_root/xremap/templates/xremap_personal_info_block.tmpl"
    XDG_CONFIG_HOME="$fixture_root" cargo run --quiet -- validate
    XDG_CONFIG_HOME="$fixture_root" cargo run --quiet -- xremap layouts
    XDG_CONFIG_HOME="$fixture_root" cargo run --quiet -- xremap generate
    XDG_CONFIG_HOME="$fixture_root" cargo run --quiet -- xremap generate --check
    if grep -Eq 'person@example|555 0100|Example Company|100 Example Avenue' "$fixture_root/xremap/config.yml"; then
        echo "generated xremap configuration contains a personal-information value" >&2
        exit 1
    fi

# Run every local verification used before merging.
check: format-check test lint docs-check examples-check

# Run xretype from the working tree, forwarding arguments after `--`.
run *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "${1:-}" == "--" ]]; then
        shift
    fi
    cargo run -- "$@"

# Validate the configured user automation file with the working tree binary.
automations-check:
    cargo run --quiet -- validate

# Preview generated xremap configuration without writing it.
xremap-preview:
    cargo run --quiet -- xremap generate --dry-run

# Fail when generated xremap configuration is stale.
xremap-check:
    cargo run --quiet -- xremap generate --check

# Regenerate the configured xremap file and its backup.
xremap-generate:
    cargo run --quiet -- xremap generate

# Install xretype and xretyped into Cargo's binary directory.
install-binaries:
    cargo install --path . --force

# Install and start the xretyped systemd user service.
install-service:
    install -Dm644 contrib/systemd/xretyped.service "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/xretyped.service"
    systemctl --user daemon-reload
    systemctl --user enable --now xretyped.service

# Copy non-destructively the starter configuration and fictitious examples.
install-examples:
    #!/usr/bin/env bash
    set -euo pipefail
    config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
    mkdir -p "$config_home/xretype"
    cp -n examples/config.toml "$config_home/xretype/config.toml"
    cp -n examples/automations.yml "$config_home/xretype/automations.yml"
    cp -n examples/personal_info.json "$config_home/personal_info.json"
    chmod 600 "$config_home/personal_info.json"
    echo "Starter files installed without replacing existing configuration."

# Install binaries, starter files, and the daemon service.
install: install-binaries install-examples install-service

# Verify, install, and restart the local user service.
deploy-local: check install-binaries install-service
    systemctl --user restart xretyped.service
    xretype daemon status

# Restart the installed daemon after configuration or binary changes.
service-restart:
    systemctl --user restart xretyped.service

# Show systemd and xretype daemon status.
service-status:
    systemctl --user status xretyped.service --no-pager
    xretype daemon status

# Follow recent daemon logs. Pass a different line count as the first argument.
service-logs lines="100":
    journalctl --user -u xretyped.service -n "{{ lines }}" -f
