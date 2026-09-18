# Troubleshooting

Start with the narrowest command that reproduces the problem. xretype reports
errors with their operation context, so the final lines normally identify the
failing layer.

## Baseline checks

```bash
xretype --version
xretype validate
xretype daemon status
systemctl --user status xretyped.service
journalctl --user -u xretyped.service --since today
```

Confirm that the client and daemon report the same installed version. After
installing a new build, restart the daemon:

```bash
systemctl --user restart xretyped.service
```

## Client cannot connect to xretyped

The error includes the Unix socket path and suggests `--standalone`.

1. Check `systemctl --user status xretyped.service`.
2. Read the unit log with `journalctl --user -u xretyped.service -b`.
3. Verify that `XDG_RUNTIME_DIR` is defined in the login session.
4. Restart the unit and retry `xretype daemon status`.
5. Run a safe local action such as
   `xretype --standalone notify "standalone works"` to separate daemon/IPC
   failure from the action backend.

The socket directory is user-owned mode `0700`, and the socket is mode `0600`.
When `XDG_RUNTIME_DIR` is unavailable, xretype uses
`/tmp/xretype-runtime-UID/xretype/` and applies the same checks.

If another live xretyped already owns the socket, a second daemon refuses to
start. A stale socket from a dead process is removed during startup.

## libei input does not type

libei establishes a compositor-mediated RemoteDesktop session. Check that:

- the desktop session is Wayland, not X11;
- the portal prompt was accepted;
- the compositor supports the required remote-input path;
- the native libei and portal components are installed;
- `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` are valid in the service environment.

At login, xretyped tries to discover a `wayland-*` compositor socket when
systemd has not yet imported `WAYLAND_DISPLAY`. If the service starts before the
desktop is ready, restart it after login and inspect its journal.

Compare daemon and local behavior while a disposable text field is focused:

```bash
xretype type "daemon path"
xretype --standalone type "standalone path"
```

## ydotool input does not type

Confirm that `ydotool` is on the service's `PATH`, `ydotoold` is running, and
the current user has the permissions required by that installation. Test the
tool independently before testing xretype.

If direct text works but a key or combo fails, check the accepted names in the
[CLI reference](cli-reference.md#keys-and-combinations). ydotool key actions
support unshifted ASCII characters; use `type` or `paste` for general Unicode
text.

## Paste inserts old or no text

xretype waits for the new clipboard offer to become readable before injecting
Ctrl+V. The error `new clipboard offer did not become readable before paste
input` means that did not happen within `clipboard_serve_ms`.

Try larger values while keeping the serve time greater than the delay:

```toml
[input]
paste_delay_ms = 75
clipboard_serve_ms = 1000
```

Restart xretyped after changing `config.toml`. Also check for clipboard-manager
behavior that replaces or consumes offers unexpectedly.

Sensitive paste exclusion is advisory and implementation-dependent. If a
clipboard manager still records a sensitive value, disable its history for the
application or desktop session; xretype cannot enforce policy in an unsupported
manager.

## Workflow changes are not active

Validate the file used by the client:

```bash
xretype validate
```

Then ask the daemon to reload its configured file:

```bash
xretype daemon reload
xretype daemon status
```

An automatic reload failure preserves the prior registry. `last reload error`
in status explains why the new file was rejected. Common causes are:

- indentation or YAML syntax errors;
- a misspelled field, rejected because the schema denies unknown fields;
- a default or parameter reference with the wrong type;
- a missing required argument in a literal `run` action;
- a static workflow cycle;
- editing a different file than `automation.file`.

If `validate` and daemon results differ, make sure both processes use the same
`--config` path and restart the daemon after configuration changes.

## Personal-information lookup fails

Test the path without the daemon and without clipboard access by using direct
typing only when a safe text field is focused:

```bash
xretype --standalone info --type --file /absolute/path/info.json Contact Email
```

The JSON path is case-sensitive. Objects, arrays, and null are not scalar
results. TOML does not expand `~` in `info.file`.

Avoid printing real secret values during diagnosis. Use a temporary JSON file
with dummy content to test paths and permissions.

## Generator cannot find a marker

Both exact begin/end pairs must exist in `config.yml`; see [generated-block
markers](xremap-integration.md#generated-block-markers). Markers may be
indented, but their text and capitalization must match after leading
whitespace.

Use the smallest relevant command:

```bash
xretype xremap generate --only layouts --dry-run
xretype xremap generate --only personal-info --dry-run
```

`--only` also avoids requiring sources for the other block while diagnosing.

## Layout validation fails

Run:

```bash
xretype xremap layouts
```

Check for an empty entry list, duplicate keys within a file, duplicate
selectors or modes across files, a multi-character selector, a reserved
`layouts` mode, or a prefix entry that conflicts with the literal-prefix rule.

The overlay command resolves the layout by filename, not display name:

```text
~/.config/xremap/layouts/logic.yml -> xretype overlay show logic
```

## Personal-information generation fails

Generated menus derive each selector from the first digit or first uppercase
ASCII letter in a name. Every generated sibling selector must be unique. For
example, `Full` and `Family` conflict at the same level, while lowercase
`email` has no derivable selector.

Direct `xretype info` lookup does not impose that menu naming convention. See
[personal-information menus](xremap-integration.md#personal-information-menus)
for the full rules.

## Overlay is missing or stale

Check the layout itself first with `xretype xremap layouts`, then run:

```bash
xretype overlay show logic
xretype daemon status
xretype overlay hide
```

GTK4 layer-shell requires a compatible Wayland compositor. The overlay is
deliberately non-focusable and click-through. Only one overlay is active; a new
show request replaces the previous one.

The overlay renderer is a supervised child of the process handling the action.
State is stored beside the daemon socket. Hide and replacement signal only that
recorded child; xretype does not use process-wide `pkill` behavior.

## xremap shortcut launches do nothing

Confirm all of the following:

- xremap started with `--allow-launch=true`;
- the absolute xretype path embedded in generated launch arrays exists;
- `xretype daemon status` succeeds in the same user session;
- `config.yml` was regenerated after changing `xremap.executable`;
- xremap reloaded the changed config;
- the selected device matches the xremap service's `--device` filter.

Preview and search the generated path without writing:

```bash
xretype xremap generate --dry-run
```

Do not put real personal values in xremap configuration while debugging. The
generated design intentionally stores only lookup paths.
