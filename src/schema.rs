//! Machine-readable JSON Schemas for the hand-edited configuration files.
//!
//! Both schemas derive from the same `Deserialize` types the loader uses, so
//! their structure cannot drift from the parser. [`tune_workflow_schema`] adds
//! the few authoring-time rules `schemars` cannot infer, each mirroring a
//! `bail!` in [`crate::automation::workflow`]. Commit the rendered files under
//! `schemas/` and refresh them with `just schema-generate`; the tests here fail
//! `just check` when the committed files or the rules above fall out of step.
//!
//! The schema covers structure only. Cycles, parameter type matching,
//! `run.with` argument types, nesting depth, and conditional requirements such
//! as an `overlay` name cannot be expressed in JSON Schema and remain the
//! job of `xretype validate`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use schemars::Schema;
use serde_json::{Value, json};

use crate::{automation::WorkflowDocument, config::Config};

/// The document a schema describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaTarget {
    /// The version 1 `automations.yml` workflow document.
    Automations,
    /// The `config.toml` settings document.
    Config,
}

impl SchemaTarget {
    /// Every target, in the order `schema-generate` writes them.
    pub const ALL: [SchemaTarget; 2] = [SchemaTarget::Automations, SchemaTarget::Config];

    /// File stem under `schemas/`, without the `.schema.json` suffix.
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Automations => "automations-v1",
            Self::Config => "config",
        }
    }

    /// Build the schema for this target.
    pub fn schema(self) -> Result<Schema> {
        match self {
            Self::Automations => {
                let mut schema = schemars::schema_for!(WorkflowDocument);
                tune_workflow_schema(&mut schema);
                Ok(schema)
            }
            Self::Config => Ok(schemars::schema_for!(Config)),
        }
    }

    /// Render the schema as pretty JSON with a trailing newline.
    pub fn render(self) -> Result<String> {
        let mut rendered = serde_json::to_string_pretty(&self.schema()?)
            .context("could not serialize the JSON schema")?;
        rendered.push('\n');
        Ok(rendered)
    }

    /// Path under the crate root: `schemas/<stem>.schema.json`.
    pub fn path(self) -> PathBuf {
        Path::new("schemas").join(format!("{}.schema.json", self.file_stem()))
    }

    /// Parse a target name as accepted by `xretype schema`.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "automations" => Ok(Self::Automations),
            "config" => Ok(Self::Config),
            other => bail!("unknown schema '{other}'; expected 'automations' or 'config'"),
        }
    }
}

/// Checked-in path of `target`, anchored at the crate root so the CLI,
/// `schema-generate`, and the drift tests agree regardless of the directory.
pub fn committed_path(target: SchemaTarget) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(target.path())
}

/// Workflow and parameter names, as `validate_name` enforces them.
const NAME_PATTERN: &str = "^[a-z][a-z0-9_-]*$";

/// Add the authoring-time rules `schemars` cannot derive from field types and
/// `deny_unknown_fields` alone. Every rule here has a matching `bail!` in
/// `automation::workflow`, and `tests` pins the pair together.
fn tune_workflow_schema(schema: &mut Schema) {
    // `WorkflowRegistry::parse` rejects any version other than 1 before it
    // reads a single workflow, so a permissive integer here would bless a
    // document the loader refuses.
    set(schema, "/properties/version", "const", json!(1));

    // Names are policed by `validate_name`, which `schemars` only sees as
    // `BTreeMap<String, _>` keys.
    set(
        schema,
        "/properties/workflows",
        "propertyNames",
        json!({ "pattern": NAME_PATTERN }),
    );
    set(
        schema,
        "/$defs/WorkflowDefinition/properties/parameters",
        "propertyNames",
        json!({ "pattern": NAME_PATTERN }),
    );

    // A workflow with no action, an `info` with no path, and a `combo` with no
    // keys all reach a `bail!` during validation.
    set(
        schema,
        "/$defs/WorkflowDefinition/properties/actions",
        "minItems",
        json!(1),
    );
    set_action_items(schema, "info", "path");
    set_action_items(schema, "combo", "keys");
}

/// Give `field` of the `ActionSpec` variant tagged `action` a `minItems`.
///
/// Variants are matched on the `action` const rather than their position in
/// `oneOf`, so adding an action later cannot silently retarget this rule.
fn set_action_items(schema: &mut Schema, action: &str, field: &str) {
    let Some(variants) = schema
        .pointer_mut("/$defs/ActionSpec/oneOf")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for variant in variants {
        let matches = variant
            .pointer("/properties/action/const")
            .and_then(Value::as_str)
            == Some(action);
        if !matches {
            continue;
        }
        let Some(entry) = variant.pointer_mut(&format!("/properties/{field}")) else {
            continue;
        };
        if let Some(object) = entry.as_object_mut() {
            object.insert("minItems".to_owned(), json!(1));
        }
    }
}

/// Insert `key` into the object at `pointer`, leaving the schema untouched
/// when a future `schemars` release stops emitting that location.
fn set(schema: &mut Schema, pointer: &str, key: &str, value: Value) {
    let Some(entry) = schema.pointer_mut(pointer) else {
        return;
    };
    if let Some(object) = entry.as_object_mut() {
        object.insert(key.to_owned(), value);
    }
}

/// Build every target as `(target, rendered JSON)`, in `ALL` order.
pub fn render_all() -> Result<Vec<(SchemaTarget, String)>> {
    SchemaTarget::ALL
        .iter()
        .map(|target| Ok((*target, target.render()?)))
        .collect()
}

/// The `$defs` of a rendered schema, for tests that inspect named types.
#[cfg(test)]
pub fn defs(schema: &Value) -> &serde_json::Map<String, Value> {
    schema
        .get("$defs")
        .and_then(Value::as_object)
        .expect("generated schemas always carry $defs")
}

/// CLI entry for `xretype schema <target>`: print the rendered schema so it
/// can be redirected into an editor configuration.
pub fn print(target: &str) -> Result<()> {
    let target = SchemaTarget::parse(target)?;
    print!("{}", target.render()?);
    Ok(())
}

/// CLI entry for `xretype schema --write`, used by `just schema-generate`:
/// refresh every committed file under `schemas/`.
pub fn write_all() -> Result<()> {
    for (target, rendered) in render_all()? {
        let path = committed_path(target);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        std::fs::write(&path, &rendered)
            .with_context(|| format!("could not write {}", path.display()))?;
        println!("wrote {}", target.path().display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::WorkflowRegistry;
    use jsonschema::Validator;

    /// A schema the loader would reject is worse than no schema: each case
    /// below must fail on *both* sides, or the pair has drifted apart.
    fn assert_agrees(loader_yaml: &str, valid_document: bool) {
        let value: Value =
            serde_saphyr::from_str(loader_yaml).expect("case must be parseable YAML");
        let schema_value = SchemaTarget::Automations
            .schema()
            .expect("automations schema must build")
            .to_value();

        let validator = Validator::new(&schema_value).expect("schema must be a valid schema");
        let schema_ok = validator.is_valid(&value);
        let loader_ok = WorkflowRegistry::parse(loader_yaml).is_ok();

        assert_eq!(
            schema_ok, loader_ok,
            "schema says valid={schema_ok} but loader says valid={loader_ok} for:\n{loader_yaml}"
        );
        assert_eq!(
            schema_ok, valid_document,
            "expected valid={valid_document}, schema said {schema_ok} for:\n{loader_yaml}"
        );
    }

    const VALID: &str = r#"
version: 1
workflows:
  hello:
    parameters:
      target:
        type: string
        default: world
    actions:
      - action: paste
        text: "hi {target}"
      - action: combo
        keys: [ctrl, shift, t]
      - action: info
        path: [Contact, Email]
      - action: sleep
        milliseconds: 10
"#;

    #[test]
    fn schema_and_loader_agree_on_every_tuned_rule() {
        assert_agrees(VALID, true);

        // `version` is pinned to 1 by `WorkflowRegistry::parse`.
        assert_agrees("version: 2\nworkflows: {}\n", false);

        // Both documents carry a field `deny_unknown_fields` refuses.
        assert_agrees("version: 1\nunknown: true\nworkflows: {}\n", false);
        assert_agrees(
            "version: 1\nworkflows:\n  w:\n    actions: []\n    typo: 1\n",
            false,
        );

        // `WorkflowDefinition::actions` must be non-empty.
        assert_agrees("version: 1\nworkflows:\n  w:\n    actions: []\n", false);

        // `validate_name` polices workflow and parameter keys.
        assert_agrees(
            "version: 1\nworkflows:\n  MyWorkflow:\n    actions: []\n",
            false,
        );
        assert_agrees(
            "version: 1\nworkflows:\n  w:\n    parameters:\n      Bad.Name:\n        type: string\n    actions: []\n",
            false,
        );

        // `validate_action` rejects an empty info path and empty combo keys.
        assert_agrees(
            "version: 1\nworkflows:\n  w:\n    actions:\n      - action: info\n        path: []\n",
            false,
        );
        assert_agrees(
            "version: 1\nworkflows:\n  w:\n    actions:\n      - action: combo\n        keys: []\n",
            false,
        );

        // Structural mistakes the schema must catch before a run exists.
        assert_agrees(
            "version: 1\nworkflows:\n  w:\n    actions:\n      - action: nope\n",
            false,
        );
        assert_agrees("version: 1\nworkflows:\n  w:\n", false);
    }

    #[test]
    fn checked_in_examples_validate_against_their_schemas() {
        for (target, relative) in [
            (SchemaTarget::Automations, "examples/automations.yml"),
            (SchemaTarget::Config, "examples/config.toml"),
        ] {
            let source =
                std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
                    .unwrap_or_else(|error| panic!("could not read {relative}: {error}"));

            let document: Value = match target {
                SchemaTarget::Automations => {
                    serde_saphyr::from_str(&source).expect("example YAML must parse")
                }
                SchemaTarget::Config => toml::from_str(&source).expect("example TOML must parse"),
            };

            let validator = Validator::new(&target.schema().unwrap().to_value())
                .expect("schema must be a valid schema");
            let errors: Vec<String> = validator
                .iter_errors(&document)
                .map(|error| format!("{}: {error}", error.instance_path()))
                .collect();

            assert!(
                errors.is_empty(),
                "{relative} must validate against {}: {}",
                target.path().display(),
                errors.join("; ")
            );
        }
    }

    #[test]
    fn committed_schemas_are_current() {
        for target in SchemaTarget::ALL {
            let rendered = target.render().expect("schema must render");
            let path = committed_path(target);
            let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "could not read {}: {error}; run `just schema-generate`",
                    path.display()
                )
            });
            assert_eq!(
                committed,
                rendered,
                "{} is stale; run `just schema-generate`",
                target.path().display()
            );
        }
    }

    #[test]
    fn tunings_land_where_schemars_emits_them() {
        // If a schemars upgrade renames `$defs` or reshapes `oneOf`, the
        // silent `set()` skips would quietly drop every rule. Fail loudly.
        let schema = SchemaTarget::Automations
            .schema()
            .expect("automations schema must build")
            .to_value();

        assert_eq!(
            schema.pointer("/properties/version/const"),
            Some(&json!(1)),
            "version pin did not land"
        );
        assert_eq!(
            schema.pointer("/properties/workflows/propertyNames/pattern"),
            Some(&json!(NAME_PATTERN)),
            "workflow name pattern did not land"
        );
        assert_eq!(
            schema.pointer("/$defs/WorkflowDefinition/properties/actions/minItems"),
            Some(&json!(1)),
            "actions minItems did not land"
        );
        assert_eq!(
            schema.pointer("/$defs/WorkflowDefinition/properties/parameters/propertyNames/pattern"),
            Some(&json!(NAME_PATTERN)),
            "parameter name pattern did not land"
        );

        let variants = defs(&schema)
            .get("ActionSpec")
            .and_then(|action| action.get("oneOf"))
            .and_then(Value::as_array)
            .expect("ActionSpec must be a oneOf");
        for (action, field) in [("info", "path"), ("combo", "keys")] {
            let variant = variants
                .iter()
                .find(|variant| variant.pointer("/properties/action/const") == Some(&json!(action)))
                .unwrap_or_else(|| panic!("no {action} variant"));
            assert_eq!(
                variant.pointer(&format!("/properties/{field}/minItems")),
                Some(&json!(1)),
                "{action}.{field} minItems did not land"
            );
        }
    }

    #[test]
    fn unknown_targets_are_rejected() {
        assert!(SchemaTarget::parse("automations").is_ok());
        assert!(SchemaTarget::parse("config").is_ok());
        assert!(SchemaTarget::parse("personal-info").is_err());
        assert!(SchemaTarget::parse("").is_err());
    }
}
