# Repository workflow

- Run `just --list` before reaching for raw development, installation, or
  deployment commands; the `justfile` is the canonical task interface.
- Use `just check` before handing off Rust or cross-cutting changes.
- For documentation-only changes, run `just docs-check`. For workflow,
  generator, configuration, or example changes, also run `just examples-check`.
- Use `just run -- <arguments>` when exercising the working-tree CLI.
- Treat `install`, `deploy-local`, `docs-deploy`, `xremap-generate`, and
  `service-*` recipes as stateful. Run them only when the user explicitly asks
  for the corresponding installation, deployment, generation, or service
  operation.
- Keep `justfile`, `README.md`, and `docs/development.md` aligned when workflows
  change.
