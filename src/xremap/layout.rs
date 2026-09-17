use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Layout {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub selector: String,
    pub mode: String,
    pub prefix: String,
    pub entries: Vec<LayoutEntry>,
    pub columns: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutEntry {
    pub key: String,
    pub symbol: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayout {
    name: String,
    selector: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default = "default_prefix")]
    prefix: String,
    entries: Vec<LayoutEntry>,
    #[serde(default = "default_columns")]
    columns: usize,
}

fn default_columns() -> usize {
    3
}

fn default_prefix() -> String {
    "Space".to_owned()
}

impl Layout {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("could not read layout {}", path.display()))?;
        let raw: RawLayout = serde_saphyr::from_str(&text)
            .map_err(|error| anyhow::anyhow!("{error}"))
            .with_context(|| format!("could not parse layout {}", path.display()))?;
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .context("layout filename must be valid UTF-8")?
            .to_owned();
        if raw.name.trim().is_empty() {
            bail!("{}: name cannot be empty", path.display());
        }
        if raw.selector.chars().count() != 1 {
            bail!("{}: selector must be one character", path.display());
        }
        if raw.entries.is_empty() {
            bail!("{}: entries cannot be empty", path.display());
        }
        if raw.columns == 0 {
            bail!("{}: columns must be at least 1", path.display());
        }
        let mode = raw
            .mode
            .unwrap_or_else(|| format!("{}_layer", slug(&raw.name)));
        if mode.trim().is_empty() || mode == "layouts" {
            bail!("{}: invalid or reserved mode '{mode}'", path.display());
        }
        if raw.prefix.trim().is_empty() || raw.prefix.contains('-') {
            bail!("{}: prefix must be one non-combination key", path.display());
        }
        let mut keys = BTreeSet::new();
        let mut prefix_targets = BTreeSet::new();
        for (index, entry) in raw.entries.iter().enumerate() {
            if entry.key.is_empty() {
                bail!("{}: entry {} has an empty key", path.display(), index + 1);
            }
            if !keys.insert(entry.key.clone()) {
                bail!("{}: duplicate entry key '{}'", path.display(), entry.key);
            }
            if entry.key == raw.prefix {
                bail!(
                    "{}: entry key '{}' is reserved as the layout prefix",
                    path.display(),
                    raw.prefix
                );
            }
            if entry.key.starts_with(&format!("{}-", raw.prefix))
                && entry.key.len() == raw.prefix.len() + 1
            {
                bail!("{}: prefix entry has no target key", path.display());
            }
            if let Some(target) = entry.key.strip_prefix(&format!("{}-", raw.prefix)) {
                if target == raw.prefix {
                    bail!(
                        "{}: '{}-{}' is reserved for a literal prefix key",
                        path.display(),
                        raw.prefix,
                        raw.prefix
                    );
                }
                if !prefix_targets.insert(target.to_owned()) {
                    bail!(
                        "{}: duplicate prefix target key '{}'",
                        path.display(),
                        target
                    );
                }
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            id,
            name: raw.name,
            selector: raw.selector,
            mode,
            prefix: raw.prefix,
            entries: raw.entries,
            columns: raw.columns,
        })
    }

    pub fn substitutions(&self) -> BTreeMap<String, KeySubstitution> {
        let mut groups = BTreeMap::new();
        for entry in &self.entries {
            let (group, prefixed) = entry_group(&entry.key, &self.prefix);
            let substitution = groups.entry(group).or_insert_with(KeySubstitution::default);
            let symbol = display_symbol(&entry.symbol);
            if prefixed {
                substitution.prefix = symbol;
            } else {
                substitution.base = symbol;
            }
        }
        groups
    }

    pub fn prefix_target<'a>(&self, key: &'a str) -> Option<&'a str> {
        key.strip_prefix(&format!("{}-", self.prefix))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeySubstitution {
    pub base: String,
    pub prefix: String,
}

pub fn load_layouts(root: &Path) -> Result<Vec<Layout>> {
    let directory = root.join("layouts");
    let mut paths = fs::read_dir(&directory)
        .with_context(|| format!("could not read layouts directory {}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yml" | "yaml")
            )
        })
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        bail!("no layouts found in {}", directory.display());
    }
    let layouts = paths
        .iter()
        .map(|path| Layout::load(path))
        .collect::<Result<Vec<_>>>()?;
    validate_collection(&layouts)?;
    Ok(layouts)
}

fn validate_collection(layouts: &[Layout]) -> Result<()> {
    let mut selectors = BTreeMap::new();
    let mut modes = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for layout in layouts {
        if !ids.insert(layout.id.clone()) {
            bail!("duplicate layout id '{}'", layout.id);
        }
        if let Some(previous) = selectors.insert(layout.selector.clone(), layout.path.clone()) {
            bail!(
                "duplicate layout selector '{}' in {} and {}",
                layout.selector,
                previous.display(),
                layout.path.display()
            );
        }
        if let Some(previous) = modes.insert(layout.mode.clone(), layout.path.clone()) {
            bail!(
                "duplicate layout mode '{}' in {} and {}",
                layout.mode,
                previous.display(),
                layout.path.display()
            );
        }
    }
    Ok(())
}

pub fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !output.is_empty() {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    if output.is_empty() {
        "layer".to_owned()
    } else {
        output
    }
}

fn entry_group(key: &str, prefix: &str) -> (String, bool) {
    if let Some(rest) = key.strip_prefix(&format!("{prefix}-")) {
        let group = if rest.chars().count() == 1
            && rest
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            rest.to_ascii_lowercase()
        } else {
            format!("shift:{rest}")
        };
        (group, true)
    } else if key.chars().count() == 1
        && key.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
    {
        (key.to_ascii_lowercase(), false)
    } else {
        (key.to_owned(), false)
    }
}

pub fn display_symbol(value: &str) -> String {
    value.replace("\\n", "↵")
}

#[cfg(test)]
mod tests {
    use super::{Layout, LayoutEntry};

    #[test]
    fn merges_base_and_prefix_substitutions() {
        let layout = Layout {
            path: "math.yml".into(),
            id: "math".to_owned(),
            name: "Math".to_owned(),
            selector: "M".to_owned(),
            mode: "math_layer".to_owned(),
            prefix: "Space".to_owned(),
            entries: vec![
                LayoutEntry {
                    key: "a".to_owned(),
                    symbol: "α".to_owned(),
                    label: None,
                },
                LayoutEntry {
                    key: "Space-a".to_owned(),
                    symbol: "Α".to_owned(),
                    label: None,
                },
            ],
            columns: 3,
        };
        let substitutions = layout.substitutions();
        assert_eq!(substitutions["a"].base, "α");
        assert_eq!(substitutions["a"].prefix, "Α");
    }
}
