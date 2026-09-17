use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{
    actions::{Action, Runtime},
    clipboard::Sensitivity,
    visual::{Notification, OverlayOperation, OverlayRequest},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDocument {
    pub version: u32,
    #[serde(default)]
    pub workflows: BTreeMap<String, WorkflowDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDefinition {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterDefinition>,
    pub actions: Vec<ActionSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterDefinition {
    #[serde(rename = "type")]
    pub kind: ParameterType,
    #[serde(default)]
    pub default: Option<ScalarValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ScalarValue {
    Boolean(bool),
    Integer(i64),
    String(String),
}

impl ScalarValue {
    fn kind(&self) -> ParameterType {
        match self {
            Self::String(_) => ParameterType::String,
            Self::Integer(_) => ParameterType::Integer,
            Self::Boolean(_) => ParameterType::Boolean,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamReference {
    param: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringExpr {
    Param(ParamReference),
    Literal(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IntegerExpr {
    Param(ParamReference),
    Literal(i64),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BooleanExpr {
    Param(ParamReference),
    Literal(bool),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ScalarExpr {
    Param(ParamReference),
    Literal(ScalarValue),
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InfoDelivery {
    #[default]
    Paste,
    Type,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowOverlayOperation {
    Show,
    Hide,
    Toggle,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ActionSpec {
    Type {
        text: StringExpr,
    },
    Paste {
        text: StringExpr,
        #[serde(default = "false_expr")]
        public: BooleanExpr,
    },
    Info {
        path: Vec<StringExpr>,
        #[serde(default)]
        file: Option<StringExpr>,
        #[serde(default)]
        delivery: InfoDelivery,
        #[serde(default = "false_expr")]
        public: BooleanExpr,
    },
    Key {
        key: StringExpr,
    },
    Combo {
        keys: Vec<StringExpr>,
    },
    Sleep {
        milliseconds: IntegerExpr,
    },
    Notify {
        #[serde(default = "default_title_expr")]
        title: StringExpr,
        body: StringExpr,
        #[serde(default = "default_timeout_expr")]
        timeout_ms: IntegerExpr,
    },
    Overlay {
        operation: WorkflowOverlayOperation,
        #[serde(default)]
        name: Option<StringExpr>,
    },
    Run {
        workflow: StringExpr,
        #[serde(default, rename = "with")]
        arguments: BTreeMap<String, ScalarExpr>,
    },
}

fn false_expr() -> BooleanExpr {
    BooleanExpr::Literal(false)
}

fn default_title_expr() -> StringExpr {
    StringExpr::Literal("xretype".to_owned())
}

fn default_timeout_expr() -> IntegerExpr {
    IntegerExpr::Literal(1500)
}

#[derive(Debug, Clone)]
pub struct WorkflowRegistry {
    workflows: BTreeMap<String, WorkflowDefinition>,
}

impl WorkflowRegistry {
    pub fn empty() -> Self {
        Self {
            workflows: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("could not read automation file {}", path.display()))?;
        Self::parse(&text)
            .with_context(|| format!("could not parse automation file {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let document: WorkflowDocument =
            serde_saphyr::from_str(text).map_err(|error| anyhow::anyhow!("{error}"))?;
        if document.version != 1 {
            bail!(
                "unsupported automation schema version {}; expected 1",
                document.version
            );
        }
        let registry = Self {
            workflows: document.workflows,
        };
        registry.validate()?;
        Ok(registry)
    }

    pub fn workflow_count(&self) -> usize {
        self.workflows.len()
    }

    pub fn execute_strings(
        &self,
        name: &str,
        raw_arguments: &BTreeMap<String, String>,
        runtime: &mut Runtime,
    ) -> Result<()> {
        let definition = self
            .workflows
            .get(name)
            .with_context(|| format!("workflow '{name}' was not found"))?;
        let mut arguments = BTreeMap::new();
        for (key, value) in raw_arguments {
            let parameter = definition
                .parameters
                .get(key)
                .with_context(|| format!("workflow '{name}' has no parameter named '{key}'"))?;
            let value = match parameter.kind {
                ParameterType::String => ScalarValue::String(value.clone()),
                ParameterType::Integer => {
                    ScalarValue::Integer(value.parse().with_context(|| {
                        format!("parameter '{key}' for workflow '{name}' must be an integer")
                    })?)
                }
                ParameterType::Boolean => {
                    ScalarValue::Boolean(parse_bool(value).with_context(|| {
                        format!("parameter '{key}' for workflow '{name}' must be a boolean")
                    })?)
                }
            };
            arguments.insert(key.clone(), value);
        }
        let bound = bind_arguments(name, definition, arguments)?;
        self.execute_bound(name, &bound, runtime, &mut Vec::new())
    }

    fn execute_bound(
        &self,
        name: &str,
        parameters: &BTreeMap<String, ScalarValue>,
        runtime: &mut Runtime,
        stack: &mut Vec<String>,
    ) -> Result<()> {
        if stack.len() >= 32 {
            bail!("workflow nesting exceeds the maximum depth of 32");
        }
        let definition = self
            .workflows
            .get(name)
            .with_context(|| format!("workflow '{name}' was not found"))?;
        stack.push(name.to_owned());
        for (index, spec) in definition.actions.iter().enumerate() {
            let result = match spec {
                ActionSpec::Run {
                    workflow,
                    arguments,
                } => {
                    let child_name = resolve_string(workflow, parameters)?;
                    let child = self
                        .workflows
                        .get(&child_name)
                        .with_context(|| format!("workflow '{child_name}' was not found"))?;
                    let mut values = BTreeMap::new();
                    for (key, value) in arguments {
                        values.insert(key.clone(), resolve_scalar(value, parameters)?);
                    }
                    let bound = bind_arguments(&child_name, child, values)?;
                    self.execute_bound(&child_name, &bound, runtime, stack)
                }
                _ => runtime.execute(resolve_action(spec, parameters)?),
            };
            if let Err(error) = result {
                stack.pop();
                return Err(error)
                    .with_context(|| format!("workflow '{name}' action {} failed", index + 1));
            }
        }
        stack.pop();
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        for (name, workflow) in &self.workflows {
            validate_name("workflow", name)?;
            if workflow.actions.is_empty() {
                bail!("workflow '{name}' must contain at least one action");
            }
            for (parameter_name, parameter) in &workflow.parameters {
                validate_name("parameter", parameter_name)?;
                if let Some(default) = &parameter.default
                    && default.kind() != parameter.kind
                {
                    bail!(
                        "default for parameter '{parameter_name}' in workflow '{name}' has the wrong type"
                    );
                }
            }
            for (index, action) in workflow.actions.iter().enumerate() {
                self.validate_action(name, workflow, index, action)?;
            }
        }
        self.validate_cycles()
    }

    fn validate_action(
        &self,
        workflow_name: &str,
        workflow: &WorkflowDefinition,
        index: usize,
        action: &ActionSpec,
    ) -> Result<()> {
        let context = || format!("workflow '{workflow_name}' action {}", index + 1);
        match action {
            ActionSpec::Type { text } => validate_string(text, workflow).with_context(context),
            ActionSpec::Paste { text, public } => {
                validate_string(text, workflow).with_context(context)?;
                validate_bool(public, workflow).with_context(context)
            }
            ActionSpec::Info {
                path, file, public, ..
            } => {
                if path.is_empty() {
                    bail!("{}: info path cannot be empty", context());
                }
                for part in path {
                    validate_string(part, workflow).with_context(context)?;
                }
                if let Some(file) = file {
                    validate_string(file, workflow).with_context(context)?;
                }
                validate_bool(public, workflow).with_context(context)
            }
            ActionSpec::Key { key } => validate_string(key, workflow).with_context(context),
            ActionSpec::Combo { keys } => {
                if keys.is_empty() {
                    bail!("{}: combo keys cannot be empty", context());
                }
                for key in keys {
                    validate_string(key, workflow).with_context(context)?;
                }
                Ok(())
            }
            ActionSpec::Sleep { milliseconds } => {
                validate_integer(milliseconds, workflow).with_context(context)
            }
            ActionSpec::Notify {
                title,
                body,
                timeout_ms,
            } => {
                validate_string(title, workflow).with_context(context)?;
                validate_string(body, workflow).with_context(context)?;
                validate_integer(timeout_ms, workflow).with_context(context)
            }
            ActionSpec::Overlay { operation, name } => {
                if !matches!(operation, WorkflowOverlayOperation::Hide) && name.is_none() {
                    bail!("{}: overlay show/toggle requires a name", context());
                }
                if let Some(name) = name {
                    validate_string(name, workflow).with_context(context)?;
                }
                Ok(())
            }
            ActionSpec::Run {
                workflow: target,
                arguments,
            } => {
                validate_string(target, workflow).with_context(context)?;
                let StringExpr::Literal(target_name) = target else {
                    // Dynamic workflow names are type-safe at runtime, but cannot
                    // participate in load-time graph and signature validation.
                    return Ok(());
                };
                let target_definition = self.workflows.get(target_name).with_context(|| {
                    format!("{}: workflow '{target_name}' was not found", context())
                })?;
                for (argument_name, expression) in arguments {
                    let target_parameter = target_definition.parameters.get(argument_name).with_context(|| {
                        format!(
                            "{}: workflow '{target_name}' has no parameter named '{argument_name}'",
                            context()
                        )
                    })?;
                    validate_scalar(expression, workflow, target_parameter.kind)
                        .with_context(context)?;
                }
                for (parameter_name, parameter) in &target_definition.parameters {
                    if parameter.default.is_none() && !arguments.contains_key(parameter_name) {
                        bail!(
                            "{}: required parameter '{parameter_name}' for workflow '{target_name}' is missing",
                            context()
                        );
                    }
                }
                Ok(())
            }
        }
    }

    fn validate_cycles(&self) -> Result<()> {
        fn visit(
            name: &str,
            registry: &WorkflowRegistry,
            visiting: &mut Vec<String>,
            complete: &mut BTreeSet<String>,
        ) -> Result<()> {
            if complete.contains(name) {
                return Ok(());
            }
            if let Some(position) = visiting.iter().position(|item| item == name) {
                let mut cycle = visiting[position..].to_vec();
                cycle.push(name.to_owned());
                bail!("workflow cycle detected: {}", cycle.join(" -> "));
            }
            visiting.push(name.to_owned());
            let workflow = &registry.workflows[name];
            for action in &workflow.actions {
                if let ActionSpec::Run {
                    workflow: StringExpr::Literal(target),
                    ..
                } = action
                {
                    visit(target, registry, visiting, complete)?;
                }
            }
            visiting.pop();
            complete.insert(name.to_owned());
            Ok(())
        }

        let mut complete = BTreeSet::new();
        for name in self.workflows.keys() {
            visit(name, self, &mut Vec::new(), &mut complete)?;
        }
        Ok(())
    }
}

fn bind_arguments(
    name: &str,
    definition: &WorkflowDefinition,
    supplied: BTreeMap<String, ScalarValue>,
) -> Result<BTreeMap<String, ScalarValue>> {
    for key in supplied.keys() {
        if !definition.parameters.contains_key(key) {
            bail!("workflow '{name}' has no parameter named '{key}'");
        }
    }
    let mut bound = BTreeMap::new();
    for (key, parameter) in &definition.parameters {
        let value = supplied
            .get(key)
            .cloned()
            .or_else(|| parameter.default.clone())
            .with_context(|| {
                format!("required parameter '{key}' for workflow '{name}' is missing")
            })?;
        if value.kind() != parameter.kind {
            bail!("parameter '{key}' for workflow '{name}' has the wrong type");
        }
        bound.insert(key.clone(), value);
    }
    Ok(bound)
}

fn resolve_action(spec: &ActionSpec, parameters: &BTreeMap<String, ScalarValue>) -> Result<Action> {
    match spec {
        ActionSpec::Type { text } => Ok(Action::Type(resolve_string(text, parameters)?)),
        ActionSpec::Paste { text, public } => Ok(Action::Paste {
            text: resolve_string(text, parameters)?,
            sensitivity: Sensitivity::from_public(resolve_bool(public, parameters)?),
        }),
        ActionSpec::Info {
            path,
            file,
            delivery,
            public,
        } => Ok(Action::Info {
            file: file
                .as_ref()
                .map(|value| resolve_string(value, parameters).map(PathBuf::from))
                .transpose()?,
            path: path
                .iter()
                .map(|value| resolve_string(value, parameters))
                .collect::<Result<Vec<_>>>()?,
            type_text: matches!(delivery, InfoDelivery::Type),
            sensitivity: Sensitivity::from_public(resolve_bool(public, parameters)?),
        }),
        ActionSpec::Key { key } => Ok(Action::Key(resolve_string(key, parameters)?)),
        ActionSpec::Combo { keys } => Ok(Action::Combo(
            keys.iter()
                .map(|key| resolve_string(key, parameters))
                .collect::<Result<Vec<_>>>()?,
        )),
        ActionSpec::Sleep { milliseconds } => Ok(Action::Sleep(nonnegative_u64(
            "milliseconds",
            resolve_integer(milliseconds, parameters)?,
        )?)),
        ActionSpec::Notify {
            title,
            body,
            timeout_ms,
        } => Ok(Action::Notify(Notification {
            title: resolve_string(title, parameters)?,
            body: resolve_string(body, parameters)?,
            timeout_ms: u32::try_from(nonnegative_u64(
                "timeout_ms",
                resolve_integer(timeout_ms, parameters)?,
            )?)
            .context("timeout_ms is too large")?,
        })),
        ActionSpec::Overlay { operation, name } => Ok(Action::Overlay(OverlayRequest {
            operation: match operation {
                WorkflowOverlayOperation::Show => OverlayOperation::Show,
                WorkflowOverlayOperation::Hide => OverlayOperation::Hide,
                WorkflowOverlayOperation::Toggle => OverlayOperation::Toggle,
            },
            name: name
                .as_ref()
                .map(|name| resolve_string(name, parameters))
                .transpose()?,
        })),
        ActionSpec::Run { .. } => {
            bail!("nested run action must be handled by the workflow runtime")
        }
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("expected true or false"),
    }
}

fn nonnegative_u64(field: &str, value: i64) -> Result<u64> {
    u64::try_from(value).with_context(|| format!("{field} cannot be negative"))
}

fn validate_name(kind: &str, name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        && name.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        });
    if !valid {
        bail!("invalid {kind} name '{name}'; use lowercase letters, digits, '-' and '_'");
    }
    Ok(())
}

fn parameter_kind(
    reference: &ParamReference,
    workflow: &WorkflowDefinition,
) -> Result<ParameterType> {
    workflow
        .parameters
        .get(&reference.param)
        .map(|definition| definition.kind)
        .with_context(|| format!("parameter '{}' was not declared", reference.param))
}

fn validate_string(expression: &StringExpr, workflow: &WorkflowDefinition) -> Result<()> {
    if let StringExpr::Param(reference) = expression
        && parameter_kind(reference, workflow)? != ParameterType::String
    {
        bail!("parameter '{}' is not a string", reference.param);
    }
    Ok(())
}

fn validate_integer(expression: &IntegerExpr, workflow: &WorkflowDefinition) -> Result<()> {
    if let IntegerExpr::Param(reference) = expression
        && parameter_kind(reference, workflow)? != ParameterType::Integer
    {
        bail!("parameter '{}' is not an integer", reference.param);
    }
    Ok(())
}

fn validate_bool(expression: &BooleanExpr, workflow: &WorkflowDefinition) -> Result<()> {
    if let BooleanExpr::Param(reference) = expression
        && parameter_kind(reference, workflow)? != ParameterType::Boolean
    {
        bail!("parameter '{}' is not a boolean", reference.param);
    }
    Ok(())
}

fn validate_scalar(
    expression: &ScalarExpr,
    workflow: &WorkflowDefinition,
    expected: ParameterType,
) -> Result<()> {
    let actual = match expression {
        ScalarExpr::Param(reference) => parameter_kind(reference, workflow)?,
        ScalarExpr::Literal(value) => value.kind(),
    };
    if actual != expected {
        bail!("workflow argument has the wrong type");
    }
    Ok(())
}

fn lookup<'a>(
    reference: &ParamReference,
    parameters: &'a BTreeMap<String, ScalarValue>,
) -> Result<&'a ScalarValue> {
    parameters
        .get(&reference.param)
        .with_context(|| format!("parameter '{}' was not bound", reference.param))
}

fn resolve_string(
    expression: &StringExpr,
    parameters: &BTreeMap<String, ScalarValue>,
) -> Result<String> {
    match expression {
        StringExpr::Literal(value) => Ok(value.clone()),
        StringExpr::Param(reference) => match lookup(reference, parameters)? {
            ScalarValue::String(value) => Ok(value.clone()),
            _ => bail!("parameter '{}' is not a string", reference.param),
        },
    }
}

fn resolve_integer(
    expression: &IntegerExpr,
    parameters: &BTreeMap<String, ScalarValue>,
) -> Result<i64> {
    match expression {
        IntegerExpr::Literal(value) => Ok(*value),
        IntegerExpr::Param(reference) => match lookup(reference, parameters)? {
            ScalarValue::Integer(value) => Ok(*value),
            _ => bail!("parameter '{}' is not an integer", reference.param),
        },
    }
}

fn resolve_bool(
    expression: &BooleanExpr,
    parameters: &BTreeMap<String, ScalarValue>,
) -> Result<bool> {
    match expression {
        BooleanExpr::Literal(value) => Ok(*value),
        BooleanExpr::Param(reference) => match lookup(reference, parameters)? {
            ScalarValue::Boolean(value) => Ok(*value),
            _ => bail!("parameter '{}' is not a boolean", reference.param),
        },
    }
}

fn resolve_scalar(
    expression: &ScalarExpr,
    parameters: &BTreeMap<String, ScalarValue>,
) -> Result<ScalarValue> {
    match expression {
        ScalarExpr::Literal(value) => Ok(value.clone()),
        ScalarExpr::Param(reference) => Ok(lookup(reference, parameters)?.clone()),
    }
}

pub fn default_automation_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("xretype/automations.yml")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{ParameterType, ScalarValue, WorkflowRegistry};

    const VALID: &str = r#"
version: 1
workflows:
  child:
    parameters:
      text: { type: string }
    actions:
      - action: type
        text: { param: text }
  parent:
    parameters:
      value: { type: string }
      delay: { type: integer, default: 5 }
      public: { type: boolean, default: false }
    actions:
      - action: sleep
        milliseconds: { param: delay }
      - action: run
        workflow: child
        with:
          text: { param: value }
"#;

    #[test]
    fn parses_typed_parameters_and_composition() {
        let registry = WorkflowRegistry::parse(VALID).unwrap();
        assert_eq!(registry.workflow_count(), 2);
        let parent = &registry.workflows["parent"];
        assert_eq!(parent.parameters["value"].kind, ParameterType::String);
        assert_eq!(
            parent.parameters["delay"].default,
            Some(ScalarValue::Integer(5))
        );
    }

    #[test]
    fn rejects_wrong_reference_type_and_cycles() {
        let wrong = VALID.replace(
            "milliseconds: { param: delay }",
            "milliseconds: { param: value }",
        );
        assert!(
            format!("{:#}", WorkflowRegistry::parse(&wrong).unwrap_err())
                .contains("not an integer")
        );

        let cycle = r#"
version: 1
workflows:
  first:
    actions: [{ action: run, workflow: second }]
  second:
    actions: [{ action: run, workflow: first }]
"#;
        assert!(
            WorkflowRegistry::parse(cycle)
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );
    }

    #[test]
    fn rejects_schema_and_unknown_fields() {
        assert!(WorkflowRegistry::parse("version: 2\nworkflows: {}").is_err());
        assert!(WorkflowRegistry::parse("version: 1\nunknown: true\nworkflows: {}").is_err());
    }

    #[test]
    fn bind_errors_do_not_include_parameter_values() {
        let registry = WorkflowRegistry::parse(VALID).unwrap();
        let mut raw = BTreeMap::new();
        raw.insert("secret".to_owned(), "do-not-log-this".to_owned());
        let error = registry
            .workflows
            .get("parent")
            .and_then(|definition| {
                super::bind_arguments(
                    "parent",
                    definition,
                    raw.into_iter()
                        .map(|(key, value)| (key, ScalarValue::String(value)))
                        .collect(),
                )
                .err()
            })
            .unwrap();
        assert!(!error.to_string().contains("do-not-log-this"));
    }
}
