# xretype Roadmap: Keyboard-First Wayland Automation

## Summary

Build xretype into a safe, keyboard-first automation runtime for Linux/Wayland
power users. xremap remains responsible for keys, devices, applications,
windows, and modes; xretype owns searchable actions, interactive sessions,
protected data, transformations, integrations, and execution.

The flagship experience is:

1. Press a shortcut.
2. A non-focus-stealing palette opens.
3. Search workflows, snippets, symbols, layouts, or personal-data labels.
4. Fill multi-tabstop snippet fields in the overlay.
5. Commit one protected paste into the still-focused application.

The palette is the same commit surface for AI-assisted flows: a shortcut
captures a region or reads the clipboard, extracts text, asks a configured
model, and commits one selected candidate through that identical protected
paste. Capture, voice, and suggestion features are built on the generic
sources, transforms, and capability grants described below rather than on a
vendor-specific core.

Use dependency-ordered releases without speculative dates.

## Release Roadmap

### 0.2 — Community-ready foundation

- Add full CI around `just check`, minimum-supported Rust, stable Rust,
  documentation, and examples.
- Introduce machine-readable schemas, shell completions, changelog/versioning
  policy, and `doctor`, `catalog list`, `catalog describe`, and redacted
  `explain` commands.
- Add a catalog model with stable namespaced IDs for workflows, snippets,
  layout entries, and personal-information paths.
- Extend IPC for interactive sessions, cancellation, catalog queries, and
  structured status; retain explicit protocol-version failures. Define an
  asynchronous single-shot action policy alongside them: which actions may
  block the serialized worker, timeouts, progress notifications, and
  stale-result discard, so long capture, OCR, transcription, and model calls
  never delay queued input actions.
- Prototype stock-xremap key forwarding through generated per-key launch
  mappings. Serialize active-session input, coalesce repeats, reject stale
  session IDs, and make zero-loss/zero-reordering stress tests a release gate.
- Track release-event forwarding as part of the same gate: hold-to-talk
  dictation ships only if it lands, while toggle dictation needs launch
  mappings alone.
- If stock transport fails the gate, automatically move the flagship to a
  pinned companion xremap runner rather than shipping unreliable input.

### 0.3 — Flagship palette and structured snippets

- Add automation schema v2 with top-level `workflows`, `snippets`, and palette
  metadata. Continue loading v1 unchanged and provide
  `xretype migrate --to 2`.
- Support a documented TextMate-style snippet subset:
  - Numbered tabstops and mirrored fields.
  - Defaults and `$0` final-caret placement.
  - Selected-text insertion/wrapping.
  - Strict parse-time validation.
- Implement daemon-owned snippet sessions:
  - Overlay-buffered editing is the reliable default.
  - Tab/Shift-Tab navigate fields, Ctrl+Enter commits, and Escape cancels.
  - Commit performs one protected paste and positions the final caret.
  - Session timeout, daemon failure, or cancellation clears buffered sensitive
    content.
- Build a fuzzy-search palette indexing workflow names, snippets, layout
  symbols, and personal-information path labels. Never index or display
  personal values.
- Generate an xremap session mode for printable keys, navigation, commit, and
  cancellation. Explicit exit mappings must return xremap to its default mode
  even if xretyped fails.
- Officially support the palette on KDE and compatible layer-shell/wlroots
  compositors. Unsupported compositors retain CLI/workflow functionality and
  receive a precise diagnostic.
- Add opt-in inline snippet adapters behind an interface; keep
  overlay-buffered execution as the supported cross-application behavior.

### 0.3.5 — Capture and voice

- Add capture, OCR, and speech-to-text backends behind a fixed-binary
  interface: region capture, screenshot, tesseract-class OCR, PipeWire audio
  capture, and a whisper-class recognizer. Backend executables are declared in
  TOML exactly as `input.backend` selects ydotool today; they are
  xretype-invoked tools, not workflow-authored shell.
- Ship side-effecting actions that need no dataflow: `capture` (region,
  window, screen, or active window), `ocr`, and dictation session commands.
  OCR and dictation results deliver through the existing protected-paste path
  and are sensitive by default.
- Ship dictation as a toggle bound to a generated xremap launch mapping.
  Hold-to-talk follows only if the 0.2 transport gate lands release-event
  forwarding.
- Add a persistent state indicator on the existing non-focus-stealing overlay
  or notification path that always reflects real capture and microphone state:
  recording, transcribing, finished, or failed. Escape cancellation or a
  daemon restart must release the microphone and discard buffered audio.
- Store capture artifacts only in `$XDG_RUNTIME_DIR`, delete them when the
  action completes or fails, and never write them to logs, status, errors, or
  history.
- Extend `doctor` with backend presence, microphone access, and compositor
  detection. Unsupported compositors receive the same precise diagnostic
  pattern as an unsupported palette.
- Extend `xremap generate` with a capture, OCR, and dictation hotkey block
  beside the existing layout and personal-information blocks, so new
  installations get Omarchy-style defaults: region OCR to clipboard,
  dictation toggle, and a capture menu.
- Add CLI parity: `xretype ocr --region`, `xretype capture --window`, and
  `xretype dictation toggle|status`.

### 0.4 — Smart paste and workflow dataflow

- Introduce typed action results and variables so actions can feed later
  actions.
- Add value sources for selection, clipboard, parameters, personal
  information, focused window title, application identifier, active URL,
  date/time, UUID, prior action output, and structured JSON.
- Add pure transforms for trimming, case conversion, regex replacement,
  split/join, wrapping, date formatting, JSON extraction/formatting, URL
  encoding, shell quoting, and Markdown/LaTeX escaping.
- Add safe interpolation and palette-generated parameter forms.
- Add `if`/`match`, bounded `for_each`, timeout, retry, and `finally`; retain
  deterministic ordering, forbid unbounded loops, and keep input-changing
  actions serialized.
- Track `public`, `sensitive`, and the `captured` and `spoken` provenance
  classes through variables and transforms. Screen text and recorded speech
  start sensitive at minimum, keep their class through every transform, and
  redact in previews, errors, history, and status.
- Preserve the prior clipboard during selection capture and restore it only
  when the user has not copied something newer.
- Keep smart paste explicit through named bindings or palette actions; never
  intercept ordinary Ctrl+V globally.

### 0.5 — Capability-gated integrations and context

- Add default-deny capabilities declared by automations and granted through
  exact config allowlists.
- Ship desktop-local integrations first:
  - Open URI/file with allowed schemes and roots.
  - Freedesktop Secret Service lookup.
  - Targeted D-Bus method calls.
- Ship HTTP actions with exact host allowlists, request timeouts, size limits,
  JSON extraction, and redacted diagnostics.
- Add a generic `ask` action on the HTTP capability: prompt, model, and
  response extraction under the same allowlist, timeout, and size controls, so
  model calls remain ordinary allowlisted requests instead of a
  vendor-specific core.
- Require a separate exact-host grant before sensitive values may leave the
  machine; deny accidental secret-to-HTTP flows during validation. Captured
  screen text and recorded speech count as sensitive for this rule, so a
  screen-derived prompt is denied until its exact host is granted.
- Add context profiles whose app, window, device, and mode filters are
  compiled into xremap configuration, with only a context ID passed to
  xretype. This builds on xremap's existing filtering and mode
  model. [xremap configuration](https://github.com/xremap/xremap)
- Read runtime context for those profiles and for value sources through
  compositor-specific sources (Hyprland IPC, sway tree, desktop portals,
  ext-foreign-toplevel), with a precise diagnostic where the compositor cannot
  report it.
- Enable context-specific palettes and workflows such as writing, browser,
  terminal, presentation, and form-filling profiles.
- State explicitly that configured backend binaries xretype invokes are not
  workflow process execution.
- Do not add arbitrary user-authored shell/process execution before 1.0.

### 0.6 — Suggest: read the screen, pick a candidate, paste

- Give the palette a candidate mode beside catalog search. Externally supplied
  candidate lists use the same navigation, commit, and cancellation path, and
  both end in one protected paste.
- Implement the suggestion flow: source (region or window capture, primary
  selection, or clipboard) → OCR when the source is an image → prompt from the
  catalog → N candidates → pick → protected paste. Cancellation, timeout, or
  daemon restart discards every buffered intermediate value.
- Run candidate generation through the 0.5 `ask` action and its HTTP grants.
  Screen-derived prompts require the separate exact-host grant, enforced at
  validation and at execution.
- Select prompts per 0.5 context profile, so writing, terminal, browser, and
  form-filling contexts offer different candidate sets.
- Keep generation asynchronous per the 0.2 policy: suggest never blocks queued
  type or paste actions, and stale results are discarded.
- Add latency budgets beside the 100 ms palette gate: end-to-end p95 on
  reference hardware for local and remote model profiles.
- Add starter configurations: rewrite selection, explain this error, draft a
  reply from this screen, and fill this form from OCR.

### 0.9 — Stabilization and distribution

- Freeze the automation v2, snippet, catalog, capability, capture,
  transcription, `ask`, and IPC contracts.
- Add migration previews, config formatting, schema-aware editor setup, and
  actionable compatibility diagnostics.
- Validate palette/snippet behavior in GTK, Qt, Chromium/Electron, terminals,
  and representative KDE/wlroots compositors.
- Publish repeatable tagged releases with checksums, Cargo installation, Arch
  packaging, and dependency-declaring Debian/Fedora packages.
- Document the compositor/backend support matrix, including capture, OCR, and
  transcription backends, and retain a runtime-only fallback where the palette
  cannot be rendered safely.
- Provide complete starter configurations for LaTeX/math/logic, protected form
  filling, selection transformations, API-backed smart paste, screen OCR,
  dictation, and screen-to-suggestion flows.

### 1.0 — Stable core product

Declare 1.0 when the palette, hybrid snippets, smart-paste dataflow, capability
enforcement, capture and voice actions, the suggestion flow, diagnostics,
migration path, and release pipeline are stable. Continue accepting workflow v1
while recommending v2.

## Public Interfaces

- `automations.yml` v2 adds snippets, palettes, context profiles, typed
  outputs, expressions, and capability declarations; existing layout YAML
  files remain separate.
- `capture`, `ocr`, and dictation actions are available to both v1 and v2
  automations once 0.3.5 ships, with sensitive delivery by default.
- Runtime values carry type, sensitivity, and provenance metadata.
- New configuration sections: `[capture]`, `[ocr]`, and `[stt]`, each naming
  its backend executables and timeouts.
- New CLI families:
  - `catalog list|describe|search`
  - `palette open`
  - `snippet run`
  - `session input|next|previous|commit|cancel|status`
  - `workflow preview|explain`
  - `permissions check`
  - `ocr`, `capture`, and `dictation toggle|status`
  - `migrate`
  - `doctor`
- `xremap generate` gains context, interactive-session, and capture/voice
  hotkey sections while continuing to preserve all text outside generated
  markers.
- Only one interactive palette/snippet session may be active; ordinary queued
  workflows remain serialized.
- Stock xremap is the first transport. Its experimental Rust plugin API is not
  a foundational dependency unless the launch transport fails its reliability
  gate. [xremap scripting status](https://github.com/xremap/xremap/blob/master/doc/reference_scripting.md)

## Acceptance and Test Plan

- Validate strict v1/v2 parsing, migration idempotence, catalog-ID uniqueness,
  snippet cycles/mirrors, malformed tabstops, and context conflicts.
- Stress the stock-xremap bridge with long bursts, repeats, cancellation,
  daemon restarts, stale clients, and concurrent launches; accept no lost,
  duplicated, or reordered input. Repeat the gate with dictation hotkey bursts
  once the transport decision lands.
- Verify full snippets with selection wrapping, Unicode pasted into fields,
  defaults, mirrored tabstops, final-caret placement, and safe cancellation.
- Confirm Escape always restores the default xremap mode even when the daemon
  or overlay is unavailable.
- Test clipboard restoration against concurrent user copies and ensure
  sensitive values never appear in logs, process arguments, status, palette
  results, or error messages.
- Confirm capture artifacts exist only under `$XDG_RUNTIME_DIR`, are deleted
  on success, failure, and cancellation, and never reach logs or history.
- Confirm captured screen text and recorded speech keep their provenance
  through variables and transforms, and that HTTP validation and execution
  both deny them without the separate exact-host grant.
- Confirm the microphone is released and buffered audio discarded on Escape,
  session timeout, and daemon restart, and that the state indicator never
  reports recording while the microphone is idle.
- Test sensitivity propagation and deny undeclared capabilities,
  disallowed paths/schemes/hosts, and secret exfiltration.
- Use mock D-Bus, Secret Service, and HTTP endpoints for deterministic
  integration tests.
- Set responsiveness gates on reference hardware: palette visible within
  100 ms, search updates within 50 ms for at least 1,000 catalog items, region
  OCR and first-transcript-after-stop within a documented budget, and the full
  suggest flow within p95 budgets for local and remote model profiles.
- Run clean-machine installation and upgrade tests for every supported package
  format.

## Post-1.0 Expansion

- Namespaced reusable packs with manifests, capability review, imports, and
  lockfiles.
- Native editor adapters for truly inline tabstops.
- GNOME-specific no-focus palette integration.
- Scheduled, clipboard, file, login/wake, and device-event triggers.
- Allowlisted process execution and reusable developer-tool packs, including
  handing instructions to local coding agents such as opencode, claude, or
  codex, either directly through that capability or through a local gateway
  using the existing HTTP capability.
- Translation, rewriting, summarization, and other AI workflows beyond the
  shipped `ask` and suggest flows, still built on the generic HTTP capability
  rather than a vendor-specific core.
- Continuous screen and audio memory integrations, such as screenpipe-style
  recall, as companion services rather than core features.
- Workflow recording, richer debugging traces, shared community catalogs, and
  additional compositor backends.
- Cross-platform work remains outside the committed roadmap until the
  Wayland/xremap product is mature.
