# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Shell completion generation for Bash, Elvish, Fish, PowerShell, and Zsh.
- A documented semantic-versioning policy and release-notes template.
- Machine-readable JSON Schemas for configuration and workflows.
- `doctor` diagnostics for the local Wayland session, configuration, and daemon.

### Fixed

- Preserve shifted keys and modifier transparency in generated xremap layouts.

## [0.1.0] - 2026-09-17

### Added

- Native typing, protected clipboard paste, key, combo, delay, notification,
  and overlay actions.
- Version 1 YAML workflows with parameters and nested workflow calls.
- A persistent per-user daemon with a private Unix socket and serialized action
  execution.
- xremap layout and personal-information block generation.
- Example configuration, systemd units, mdBook documentation, and `just`
  development workflows.

