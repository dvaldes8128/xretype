use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub fn resolve(file: &Path, path: &[String]) -> Result<String> {
    let contents = fs::read_to_string(file)
        .with_context(|| format!("could not read JSON file {}", file.display()))?;
    let document: Value = serde_json::from_str(&contents)
        .with_context(|| format!("could not parse JSON file {}", file.display()))?;

    let display_path = path.join(".");
    let mut current = &document;
    for key in path {
        current = current
            .get(key)
            .with_context(|| format!("JSON path '{display_path}' was not found"))?;
    }

    match current {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => bail!("JSON path '{display_path}' is null"),
        Value::Array(_) | Value::Object(_) => {
            bail!("JSON path '{display_path}' is not a scalar value")
        }
    }
}

pub fn normalize_path(arguments: &[String]) -> Result<Vec<String>> {
    let path = arguments
        .iter()
        .flat_map(|argument| argument.split('.'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if path.is_empty() {
        bail!("JSON path cannot be empty");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{normalize_path, resolve};

    fn fixture() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("xretype-{nonce}.json"));
        fs::write(
            &path,
            r#"{"identity":{"email":"person@example.com"},"age":42}"#,
        )
        .unwrap();
        path
    }

    #[test]
    fn normalizes_dotted_and_separate_segments() {
        let arguments = vec!["identity.email".to_owned()];
        assert_eq!(normalize_path(&arguments).unwrap(), ["identity", "email"]);

        let arguments = vec!["identity".to_owned(), "email".to_owned()];
        assert_eq!(normalize_path(&arguments).unwrap(), ["identity", "email"]);
    }

    #[test]
    fn resolves_string_and_number_scalars() {
        let file = fixture();
        assert_eq!(
            resolve(&file, &["identity".into(), "email".into()]).unwrap(),
            "person@example.com"
        );
        assert_eq!(resolve(&file, &["age".into()]).unwrap(), "42");
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn missing_path_is_an_error() {
        let file = fixture();
        assert!(resolve(&file, &["missing".into()]).is_err());
        fs::remove_file(file).unwrap();
    }
}
