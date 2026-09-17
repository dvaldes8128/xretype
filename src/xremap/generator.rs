use std::{
    collections::{BTreeMap, VecDeque},
    env, fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

use crate::{
    config::Config,
    xremap::layout::{Layout, load_layouts, slug},
};

const OLD_LAYOUTS_BEGIN: &str =
    "# BEGIN GENERATED LAYOUTS (run: scripts/generate_xremap_layouts.py)";
const LAYOUTS_BEGIN: &str =
    "# BEGIN GENERATED LAYOUTS (run: xretype xremap generate --only layouts)";
const LAYOUTS_END: &str = "# END GENERATED LAYOUTS";
const OLD_INFO_BEGIN: &str =
    "# BEGIN GENERATED PERSONAL_INFO (run: scripts/generate_xremap_personal_info.py)";
const INFO_BEGIN: &str =
    "# BEGIN GENERATED PERSONAL_INFO (run: xretype xremap generate --only personal-info)";
const INFO_END: &str = "# END GENERATED PERSONAL_INFO";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerateSection {
    Layouts,
    PersonalInfo,
}

#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    pub root: Option<PathBuf>,
    pub section: Option<GenerateSection>,
    pub dry_run: bool,
    pub check: bool,
}

pub fn generate(config: &Config, options: &GenerateOptions) -> Result<bool> {
    let root = options.root.clone().unwrap_or_else(|| config.xremap_root());
    let config_path = root.join("config.yml");
    let original = fs::read_to_string(&config_path)
        .with_context(|| format!("could not read {}", config_path.display()))?;
    let executable = resolve_executable(config)?;
    let mut updated = original.clone();

    if options.section.is_none() || options.section == Some(GenerateSection::PersonalInfo) {
        updated = generate_personal_info(config, &root, &executable, &updated)?;
    }
    if options.section.is_none() || options.section == Some(GenerateSection::Layouts) {
        updated = generate_layouts(&root, &executable, &updated)?;
    }

    let changed = updated != original;
    if options.dry_run {
        print!("{updated}");
    } else if options.check {
        if changed {
            bail!("generated xremap configuration is out of date");
        }
    } else if changed {
        let backup = root.join("config.yml.bak");
        atomic_write(&backup, original.as_bytes())?;
        atomic_write(&config_path, updated.as_bytes())?;
    }
    Ok(changed)
}

fn generate_layouts(root: &Path, executable: &Path, config_text: &str) -> Result<String> {
    let layouts = load_layouts(root)?;
    let template_path = root.join("templates/xremap_layouts_block.tmpl");
    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("could not read {}", template_path.display()))?;
    let block = template
        .replace(
            "${trigger_block}",
            &render_layout_trigger(&layouts, executable),
        )
        .replace("${menu_block}", &render_layout_menu(&layouts, executable))
        .replace(
            "${layers_block}",
            &render_layout_layers(&layouts, executable),
        );
    replace_generated_block(
        config_text,
        &[OLD_LAYOUTS_BEGIN, LAYOUTS_BEGIN],
        LAYOUTS_END,
        LAYOUTS_BEGIN,
        block.trim_end(),
    )
}

fn render_layout_trigger(layouts: &[Layout], executable: &Path) -> String {
    let listing = format!(
        " {}",
        layouts
            .iter()
            .map(|layout| format!("{} - {}", layout.selector, layout.name))
            .collect::<Vec<_>>()
            .join(" \\n ")
    );
    [
        "  - name: Layouts Trigger".to_owned(),
        "    remap:".to_owned(),
        "      F22:".to_owned(),
        "        remap:".to_owned(),
        "          # Double-tap trigger enters the shared Layouts menu.".to_owned(),
        "          F22:".to_owned(),
        "            - set_mode: layouts".to_owned(),
        format!(
            "            - launch: {}",
            notify_argv(executable, "Layouts", &listing, 5000)
        ),
        "        timeout_millis: 200".to_owned(),
        "        timeout_key: F22".to_owned(),
    ]
    .join("\n")
}

fn render_layout_menu(layouts: &[Layout], executable: &Path) -> String {
    let mut lines = vec![
        "  - name: Layouts Menu".to_owned(),
        "    mode: layouts".to_owned(),
        "    remap:".to_owned(),
        "      Esc:".to_owned(),
        "        - set_mode: default".to_owned(),
    ];
    for layout in layouts {
        lines.extend([
            format!("      {}:", yaml_key(&layout.selector)),
            format!("        - set_mode: {}", layout.mode),
            format!(
                "        - launch: {}",
                json_argv(&[
                    path_string(executable),
                    "overlay".to_owned(),
                    "show".to_owned(),
                    layout.id.clone(),
                ])
            ),
        ]);
    }
    lines.join("\n")
}

fn render_layout_layers(layouts: &[Layout], executable: &Path) -> String {
    let mut blocks = Vec::new();
    for layout in layouts {
        let prefix_entries = layout
            .entries
            .iter()
            .filter_map(|entry| layout.prefix_target(&entry.key).map(|key| (key, entry)))
            .collect::<Vec<_>>();
        blocks.extend([
            String::new(),
            format!("  - name: Layout - {}", layout.name),
            format!("    mode: {}", layout.mode),
            "    remap:".to_owned(),
            "      Esc:".to_owned(),
            "        - set_mode: default".to_owned(),
            format!(
                "        - launch: {}",
                json_argv(&[
                    path_string(executable),
                    "overlay".to_owned(),
                    "hide".to_owned(),
                ])
            ),
        ]);
        if !prefix_entries.is_empty() {
            for modifier in ["Alt", "Ctrl", "Shift", "Super"] {
                let chord = format!("{modifier}-{}", layout.prefix);
                blocks.extend([format!("      {chord}:"), format!("        - {chord}")]);
            }
            blocks.extend([
                format!("      {}:", yaml_key(&layout.prefix)),
                format!("        - set_mode: {}", prefix_mode(layout)),
                format!(
                    "        - launch: {}",
                    overlay_show_argv(executable, &layout.id, "prefix")
                ),
            ]);
        }
        for entry in layout
            .entries
            .iter()
            .filter(|entry| layout.prefix_target(&entry.key).is_none())
        {
            let symbol = entry.symbol.replace("\\n", "\n");
            let alt_key = alt_passthrough_key(&entry.key);
            let alt_value = alt_passthrough_value(&entry.key);
            blocks.extend([
                format!("      {}:", yaml_key(&alt_key)),
                format!("        - {}", yaml_key(&alt_value)),
                format!("      {}:", yaml_key(&entry.key)),
                format!(
                    "        - launch: {}",
                    json_argv(&[path_string(executable), "paste".to_owned(), symbol,])
                ),
            ]);
        }
        if !prefix_entries.is_empty() {
            blocks.extend([
                String::new(),
                format!(
                    "  - name: Layout - {} ({} prefix)",
                    layout.name, layout.prefix
                ),
                format!("    mode: {}", prefix_mode(layout)),
                "    remap:".to_owned(),
                "      Esc:".to_owned(),
                format!("        - set_mode: {}", layout.mode),
                format!(
                    "        - launch: {}",
                    overlay_show_argv(executable, &layout.id, "base")
                ),
                format!("      {}:", yaml_key(&layout.prefix)),
                format!("        - set_mode: {}", layout.mode),
                format!(
                    "        - launch: {}",
                    overlay_show_argv(executable, &layout.id, "base")
                ),
                format!("        - {}", yaml_key(&layout.prefix)),
            ]);
            for (key, entry) in prefix_entries {
                let symbol = entry.symbol.replace("\\n", "\n");
                blocks.extend([
                    format!("      {}:", yaml_key(key)),
                    format!("        - set_mode: {}", layout.mode),
                    format!(
                        "        - launch: {}",
                        json_argv(&[
                            path_string(executable),
                            "paste".to_owned(),
                            "--overlay-reset".to_owned(),
                            layout.id.clone(),
                            symbol,
                        ])
                    ),
                ]);
            }
        }
    }
    blocks.join("\n").trim_start_matches('\n').to_owned()
}

fn prefix_mode(layout: &Layout) -> String {
    format!("{}_{}_prefix", layout.mode, slug(&layout.prefix))
}

fn overlay_show_argv(executable: &Path, layout: &str, view: &str) -> String {
    json_argv(&[
        path_string(executable),
        "overlay".to_owned(),
        "show".to_owned(),
        layout.to_owned(),
        "--view".to_owned(),
        view.to_owned(),
    ])
}

fn generate_personal_info(
    config: &Config,
    root: &Path,
    executable: &Path,
    config_text: &str,
) -> Result<String> {
    let info_path = config.info_file();
    let text = fs::read_to_string(&info_path)
        .with_context(|| format!("could not read {}", info_path.display()))?;
    let info: Value = serde_json::from_str(&text)
        .with_context(|| format!("could not parse {}", info_path.display()))?;
    let object = info
        .as_object()
        .context("personal information file must contain a JSON object")?;
    let categories = personal_categories(object)?;
    let template_path = root.join("templates/xremap_personal_info_block.tmpl");
    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("could not read {}", template_path.display()))?;
    let block = template
        .replace(
            "${trigger_block}",
            &render_info_trigger(&categories, executable),
        )
        .replace(
            "${layer0_block}",
            &render_info_layer_zero(&categories, object, executable),
        )
        .replace(
            "${layers_block}",
            &render_info_layers(&categories, object, executable)?,
        );
    replace_generated_block(
        config_text,
        &[OLD_INFO_BEGIN, INFO_BEGIN],
        INFO_END,
        INFO_BEGIN,
        block.trim_end(),
    )
}

#[derive(Debug, Clone)]
struct Category {
    name: String,
    selector: String,
    mode: String,
}

fn personal_categories(info: &Map<String, Value>) -> Result<Vec<Category>> {
    let mut used = BTreeMap::new();
    let mut categories = Vec::new();
    for (name, value) in info {
        if !value.is_object() {
            continue;
        }
        let selector = derived_key(name).with_context(|| {
            format!("cannot derive selector for personal-information category '{name}'")
        })?;
        if let Some(previous) = used.insert(selector.clone(), name.clone()) {
            bail!(
                "duplicate personal-information selector '{selector}' for '{previous}' and '{name}'"
            );
        }
        let mode = match name.as_str() {
            "eMail" => "mail_layer".to_owned(),
            "Name" => "name_layer".to_owned(),
            "Phone" => "phone_layer".to_owned(),
            "Orcid" => "orcid_layer".to_owned(),
            _ => format!("{}_layer", slug(name)),
        };
        categories.push(Category {
            name: name.clone(),
            selector,
            mode,
        });
    }
    Ok(categories)
}

fn render_info_trigger(categories: &[Category], executable: &Path) -> String {
    let body = format!(
        " {}",
        categories
            .iter()
            .map(|category| category.name.as_str())
            .collect::<Vec<_>>()
            .join(" \\n ")
    );
    [
        "  - name: Personal Data Injection Trigger".to_owned(),
        "    remap:".to_owned(),
        "      F23:".to_owned(),
        "        remap:".to_owned(),
        "          # If tapped twice, clear the screen".to_owned(),
        "          F23:".to_owned(),
        "            - set_mode: insert".to_owned(),
        format!(
            "            - launch: {}",
            notify_argv(executable, "Insertion modes", &body, 5000)
        ),
        "        timeout_millis: 200".to_owned(),
        "        timeout_key: F23".to_owned(),
    ]
    .join("\n")
}

fn escape_block(executable: &Path) -> Vec<String> {
    vec![
        "      Esc:".to_owned(),
        "        - set_mode: default".to_owned(),
        format!(
            "        - launch: {}",
            notify_argv(executable, "Injection mode", "OFF", 2000)
        ),
    ]
}

fn render_info_layer_zero(
    categories: &[Category],
    info: &Map<String, Value>,
    executable: &Path,
) -> String {
    let mut lines = vec![
        "  - name: Personal Data Injection Layer 0".to_owned(),
        "    mode: insert".to_owned(),
        "    remap:".to_owned(),
    ];
    lines.extend(escape_block(executable));
    for category in categories {
        let entries = info[&category.name]
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        let body = if entries.is_empty() {
            String::new()
        } else {
            format!(" {}", entries.join(" \\n "))
        };
        lines.extend([
            format!("      {}:", yaml_key(&category.selector)),
            format!("        - set_mode: {}", category.mode),
            format!(
                "        - launch: {}",
                notify_argv(
                    executable,
                    &format!("{} available", category.name),
                    &body,
                    10000,
                )
            ),
        ]);
    }
    lines.join("\n")
}

fn render_info_layers(
    categories: &[Category],
    info: &Map<String, Value>,
    executable: &Path,
) -> Result<String> {
    let mut blocks = Vec::new();
    for category in categories {
        let Some(root) = info[&category.name].as_object() else {
            continue;
        };
        let mut queue = VecDeque::from([(Vec::<String>::new(), root)]);
        while let Some((path, node)) = queue.pop_front() {
            if node.is_empty() {
                continue;
            }
            let keymap = strict_entry_keys(node.keys().map(String::as_str), &category.name)?;
            let label = if path.is_empty() {
                category.name.clone()
            } else {
                format!("{} / {}", category.name, path.join(" / "))
            };
            blocks.extend([
                String::new(),
                format!("  - name: {label} layer"),
                format!("    mode: {}", mode_for_path(category, &path)),
                "    remap:".to_owned(),
            ]);
            blocks.extend(escape_block(executable));
            for (entry_name, value) in node {
                let key = &keymap[entry_name];
                let mut child_path = path.clone();
                child_path.push(entry_name.clone());
                if value.is_string() {
                    let mut argv = vec![
                        path_string(executable),
                        "info".to_owned(),
                        category.name.clone(),
                    ];
                    argv.extend(child_path.iter().cloned());
                    blocks.extend([
                        format!("      {}:", yaml_key(key)),
                        "        - set_mode: default".to_owned(),
                        format!("        - launch: {}", json_argv(&argv)),
                    ]);
                } else if let Some(child) = value.as_object()
                    && !child.is_empty()
                {
                    queue.push_back((child_path.clone(), child));
                    let body = format!(
                        " {}",
                        child
                            .keys()
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join(" \\n ")
                    );
                    blocks.extend([
                        format!("      {}:", yaml_key(key)),
                        format!(
                            "        - set_mode: {}",
                            mode_for_path(category, &child_path)
                        ),
                        format!(
                            "        - launch: {}",
                            notify_argv(
                                executable,
                                &format!("{} / {}", category.name, child_path.join(" / ")),
                                &body,
                                10000,
                            )
                        ),
                    ]);
                }
            }
        }
    }
    Ok(blocks.join("\n").trim_start_matches('\n').to_owned())
}

fn strict_entry_keys<'a>(
    names: impl Iterator<Item = &'a str>,
    category: &str,
) -> Result<BTreeMap<String, String>> {
    let mut output = BTreeMap::new();
    let mut used = BTreeMap::new();
    for name in names {
        let key = derived_key(name).with_context(|| {
            format!("cannot derive key for entry '{name}' in category '{category}'")
        })?;
        if let Some(previous) = used.insert(key.clone(), name.to_owned()) {
            bail!(
                "duplicate entry key '{key}' in category '{category}' for '{previous}' and '{name}'"
            );
        }
        output.insert(name.to_owned(), key);
    }
    Ok(output)
}

fn derived_key(value: &str) -> Option<String> {
    value
        .chars()
        .find(char::is_ascii_digit)
        .or_else(|| value.chars().find(char::is_ascii_uppercase))
        .map(|character| character.to_string())
}

fn mode_for_path(category: &Category, path: &[String]) -> String {
    if path.is_empty() {
        category.mode.clone()
    } else {
        format!("{}_{}", category.mode, slug(&path.join("_")))
    }
}

fn replace_generated_block(
    text: &str,
    begin_markers: &[&str],
    end_marker: &str,
    new_begin: &str,
    body: &str,
) -> Result<String> {
    let mut offset = 0;
    let mut begin = None;
    let mut end = None;
    let mut indent = String::new();
    for line_with_newline in text.split_inclusive('\n') {
        let line = line_with_newline.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        if begin.is_none() && begin_markers.contains(&trimmed) {
            begin = Some(offset);
            indent = line[..line.len() - trimmed.len()].to_owned();
        } else if begin.is_some() && trimmed == end_marker {
            end = Some(offset + line.len());
            break;
        }
        offset += line_with_newline.len();
    }
    let begin = begin.context("generated block begin marker was not found")?;
    let end = end.context("generated block end marker was not found")?;
    let replacement = format!(
        "{indent}{new_begin}\n{}\n{indent}{end_marker}",
        body.trim_end()
    );
    Ok(format!("{}{}{}", &text[..begin], replacement, &text[end..]))
}

fn yaml_key(value: &str) -> String {
    if value.chars().count() == 1 && value.chars().all(|character| character.is_ascii_digit()) {
        return format!("\"{value}\"");
    }
    if value.chars().enumerate().all(|(index, character)| {
        if index == 0 {
            character.is_ascii_alphabetic() || character == '_'
        } else {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
        }
    }) {
        value.to_owned()
    } else {
        serde_json::to_string(value).expect("string serialization cannot fail")
    }
}

fn alt_passthrough_key(key: &str) -> String {
    if key.starts_with("Alt-") || key.starts_with("M-") || key.starts_with("Mod1-") {
        key.to_owned()
    } else {
        format!("Alt-{key}")
    }
}

fn alt_passthrough_value(key: &str) -> String {
    ["Alt-", "M-", "Mod1-"]
        .iter()
        .find_map(|prefix| key.strip_prefix(prefix))
        .unwrap_or(key)
        .to_owned()
}

fn notify_argv(executable: &Path, title: &str, body: &str, timeout_ms: u32) -> String {
    json_argv(&[
        path_string(executable),
        "notify".to_owned(),
        "--title".to_owned(),
        title.to_owned(),
        "--timeout-ms".to_owned(),
        timeout_ms.to_string(),
        body.replace("\\n", "\n"),
    ])
}

fn json_argv(parts: &[String]) -> String {
    format!(
        "[{}]",
        parts
            .iter()
            .map(|part| serde_json::to_string(part).expect("argument serialization cannot fail"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn resolve_executable(config: &Config) -> Result<PathBuf> {
    if let Some(path) = &config.xremap.executable {
        return Ok(path.clone());
    }
    if let Some(path) = env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join("xretype"))
            .find(|candidate| candidate.is_file())
    }) {
        return Ok(path);
    }
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".cargo/bin/xretype");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    env::current_exe().context("could not determine xretype executable path")
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("output path has no parent directory")?;
    let temporary = parent.join(format!(
        ".{}.xretype-{}-{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("output"),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .with_context(|| format!("could not create {}", temporary.display()))?;
        file.write_all(contents)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(fs::Permissions::from_mode(metadata.permissions().mode()))?;
        }
        file.sync_all()?;
        fs::rename(&temporary, path)
            .with_context(|| format!("could not replace {} atomically", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::xremap::{Layout, LayoutEntry};

    use super::{
        alt_passthrough_key, alt_passthrough_value, render_layout_layers, replace_generated_block,
        yaml_key,
    };

    #[test]
    fn replaces_old_markers_without_touching_surrounding_text() {
        let input = "before\n  # OLD\nold body\n  # END\nafter\n";
        let output =
            replace_generated_block(input, &["# OLD"], "# END", "# NEW", "new body").unwrap();
        assert_eq!(output, "before\n  # NEW\nnew body\n  # END\nafter\n");
    }

    #[test]
    fn quotes_yaml_keys_and_preserves_alt_behavior() {
        assert_eq!(yaml_key("8"), "\"8\"");
        assert_eq!(yaml_key("Shift-a"), "Shift-a");
        assert_eq!(alt_passthrough_key("Shift-a"), "Alt-Shift-a");
        assert_eq!(alt_passthrough_value("Alt-Shift-a"), "Shift-a");
    }

    #[test]
    fn renders_a_space_prefix_mode_and_preserves_alt_space() {
        let layout = Layout {
            path: "logic.yml".into(),
            id: "logic".to_owned(),
            name: "Logic".to_owned(),
            selector: "L".to_owned(),
            mode: "logic_layer".to_owned(),
            prefix: "Space".to_owned(),
            entries: vec![
                LayoutEntry {
                    key: "a".to_owned(),
                    symbol: "∧".to_owned(),
                    label: None,
                },
                LayoutEntry {
                    key: "Space-a".to_owned(),
                    symbol: "∀".to_owned(),
                    label: None,
                },
            ],
            columns: 3,
        };

        let output = render_layout_layers(&[layout], Path::new("/bin/xretype"));
        assert!(output.contains("mode: logic_layer_space_prefix"));
        assert!(output.contains("Alt-Space:\n        - Alt-Space"));
        assert!(output.contains("overlay-reset\", \"logic\", \"∀\""));
        assert!(output.contains("Space:\n        - set_mode: logic_layer"));
    }
}
