# Workflows

Workflows are named, typed sequences of the same primitive actions exposed by
the CLI. They execute one action at a time and stop at the first failure.

The default file is `~/.config/xretype/automations.yml`. The top-level schema
is strict:

```yaml
version: 1

workflows:
  example:
    description: Optional human-readable text.
    parameters: {}
    actions:
      - action: notify
        body: Hello
```

Unknown fields, an unsupported version, invalid types, empty action lists,
missing required arguments, and statically detectable call cycles are rejected.

## Parameters and expressions

A parameter has type `string`, `integer`, or `boolean`. It is required unless a
same-typed default is present:

```yaml
parameters:
  text: { type: string }
  delay-ms: { type: integer, default: 30 }
  public: { type: boolean, default: false }
```

CLI values are supplied as strings and converted according to these
declarations:

```bash
xretype run delayed-paste \
  --param text='hello' \
  --param delay-ms=50 \
  --param public=false
```

Action fields accept either a literal of the required type or an explicit
parameter reference:

```yaml
text: fixed text
milliseconds: 100
public: false

text: { param: text }
milliseconds: { param: delay-ms }
public: { param: public }
```

There is no string interpolation. A parameter reference supplies the entire
field value.

Workflow and parameter names must start with a lowercase ASCII letter. The
remaining characters may be lowercase ASCII letters, digits, `_`, or `-`.

## Actions

### Type

```yaml
- action: type
  text: Hello from xretype

- action: type
  text: { param: text }
```

Types text through the configured input backend without using the clipboard.

### Paste

```yaml
- action: paste
  text: { param: text }
  public: { param: public }
```

`public` is optional and defaults to `false`, so workflow paste is sensitive by
default. The workflow action does not expose the CLI-only `overlay-reset`
option.

### Personal information

```yaml
- action: info
  path: [Contact, Email]

- action: info
  path: [Work, Address, City]
  delivery: type

- action: info
  file: /home/you/.config/other-info.json
  path:
    - { param: category }
    - Email
  delivery: paste
  public: false
```

`path` must contain at least one string expression. `file` is optional;
omitting it uses `info.file`. `delivery` is `paste` by default or `type`.
`public` defaults to `false` and applies to paste delivery.

### Key and combo

```yaml
- action: key
  key: enter

- action: combo
  keys: [ctrl, shift, t]
```

A combo requires at least one key. Accepted names are listed in the [CLI
reference](cli-reference.md#keys-and-combinations).

### Sleep

```yaml
- action: sleep
  milliseconds: 250
```

The integer is converted to a non-negative millisecond duration at execution
time. Negative values fail before sleeping.

### Notify

```yaml
- action: notify
  title: xretype workflow
  body: Starting now
  timeout_ms: 3000
```

`title` defaults to `xretype`, and `timeout_ms` defaults to `1500`. `body` is
required.

### Overlay

```yaml
- action: overlay
  operation: show
  name: logic

- action: overlay
  operation: toggle
  name: logic

- action: overlay
  operation: hide
```

Operations are `show`, `toggle`, and `hide`. Show and toggle require a layout
name. Hide permits an optional name. Workflow overlays open in the base view;
the prefix view is available through the CLI and generated xremap actions.

### Run another workflow

```yaml
- action: run
  workflow: delayed-paste
  with:
    text: { param: text }
    delay-ms: 100
```

`with` binds values to the called workflow's parameters. Defaults still apply
to omitted bindings. When `workflow` is a literal, validation checks that the
target exists, argument names and types match, required arguments are present,
and the static call graph has no cycles.

A parameter reference may select the workflow dynamically:

```yaml
parameters:
  next: { type: string }
actions:
  - action: run
    workflow: { param: next }
```

Dynamic targets are checked at execution because their name and signature are
not known when the file loads. Runtime nesting is limited to 32 calls.

## Composition example

```yaml
version: 1

workflows:
  delayed-paste:
    parameters:
      text: { type: string }
      delay-ms: { type: integer, default: 30 }
      public: { type: boolean, default: false }
    actions:
      - action: sleep
        milliseconds: { param: delay-ms }
      - action: paste
        text: { param: text }
        public: { param: public }

  announce-and-paste:
    parameters:
      text: { type: string }
    actions:
      - action: notify
        title: xretype
        body: Pasting shortly
      - action: run
        workflow: delayed-paste
        with:
          text: { param: text }
          delay-ms: 250
```

Run it with:

```bash
xretype validate
xretype run announce-and-paste --param text='hello'
```

## Reload and failure behavior

xretyped loads the registry before accepting requests. It then watches the
workflow directory, debounces relevant filesystem changes, and parses each new
version. A valid file replaces the registry and increments the configuration
generation. An invalid file leaves the last valid registry active and records
an error visible through:

```bash
xretype daemon status
```

Use `xretype daemon reload` when an editor's save behavior is not detected or a
script needs an immediate success/failure result.

Execution errors identify the workflow and one-based action number. They do not
include resolved text or parameter values.

## Intentional version 1 limits

Version 1 has no branching, loops, retry policy, concurrency, interpolation,
output capture, or arbitrary process execution. These limits keep validation,
ordering, and sensitive-data handling predictable.
