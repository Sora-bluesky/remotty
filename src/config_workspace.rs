use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use toml::Value;
use uuid::Uuid;

const STARTER_CONFIG: &str = include_str!("../bridge.toml");
const PLACEHOLDER_WORKSPACE_PATH: &str = "C:/path/to/workspace";
const DEFAULT_MODE: &str = "await_reply";
const DEFAULT_CHECKS_PROFILE: &str = "default";
const DEFAULT_CONTINUE_PROMPT: &str =
    "Continue with the needed checks. If you must stop, reply with the short reason.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceUpsertResult {
    pub workspace_id: String,
    pub workspace_path: PathBuf,
    pub config_path: PathBuf,
    pub created_config: bool,
}

pub fn upsert_workspace(
    config_path: impl AsRef<Path>,
    workspace_path: impl AsRef<Path>,
) -> Result<WorkspaceUpsertResult> {
    let config_path = config_path.as_ref();
    let workspace_path = canonicalize_workspace(workspace_path.as_ref())?;
    let created_config = ensure_config_file(config_path)?;
    let raw = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read config file: {}", config_path.display()))?;
    let mut document = raw
        .parse::<Value>()
        .context("failed to parse bridge.toml as TOML")?;
    upsert_workspace_value(&mut document, &workspace_path)?;
    let rendered = toml::to_string_pretty(&document).context("failed to render bridge.toml")?;
    fs::write(config_path, rendered)
        .with_context(|| format!("failed to write config file: {}", config_path.display()))?;

    Ok(WorkspaceUpsertResult {
        workspace_id: document
            .get("workspaces")
            .and_then(Value::as_array)
            .and_then(|workspaces| workspaces.first())
            .and_then(Value::as_table)
            .and_then(|workspace| workspace.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("main")
            .to_owned(),
        workspace_path,
        config_path: config_path.to_path_buf(),
        created_config,
    })
}

pub fn ensure_default_workspace_is_ready(config: &crate::config::Config) -> Result<()> {
    let workspace = config.default_workspace();
    if is_placeholder_path(&workspace.path) {
        bail!(
            "workspace is not configured. Open or enter the target project and use the `remotty-use-this-project` skill or `remotty config workspace upsert`."
        );
    }
    if !workspace.path.is_dir() {
        bail!(
            "workspace path does not exist: {}. Open or enter the target project and use the `remotty-use-this-project` skill or `remotty config workspace upsert`.",
            workspace.path.display()
        );
    }
    Ok(())
}

fn ensure_config_file(config_path: &Path) -> Result<bool> {
    if config_path.exists() {
        return Ok(false);
    }
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
    }
    fs::write(config_path, STARTER_CONFIG)
        .with_context(|| format!("failed to create config file: {}", config_path.display()))?;
    Ok(true)
}

fn upsert_workspace_value(document: &mut Value, workspace_path: &Path) -> Result<()> {
    let Some(root) = document.as_table_mut() else {
        bail!("bridge.toml root must be a table");
    };
    let Some(workspaces) = root
        .entry("workspaces".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
    else {
        bail!("workspaces must be an array");
    };

    let normalized_path = render_path(workspace_path);
    let exact_index = workspaces
        .iter()
        .position(|workspace| workspace_path_matches(workspace, workspace_path));
    let placeholder_index = workspaces.iter().position(workspace_uses_placeholder_path);
    let selected_index = exact_index.or(placeholder_index);
    let mut workspace = selected_index
        .map(|index| workspaces.remove(index))
        .unwrap_or_else(|| new_workspace_table());

    let existing_id = if exact_index.is_some() {
        workspace
            .as_table()
            .and_then(|table| table.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    } else {
        None
    };
    let workspace_id = existing_id.unwrap_or_else(|| next_workspace_id(workspaces, workspace_path));
    let table = workspace
        .as_table_mut()
        .ok_or_else(|| anyhow!("workspace entry must be a table"))?;
    table.insert("id".to_owned(), Value::String(workspace_id));
    table.insert("path".to_owned(), Value::String(normalized_path.clone()));
    table.insert(
        "writable_roots".to_owned(),
        Value::Array(vec![Value::String(normalized_path)]),
    );
    table
        .entry("default_mode".to_owned())
        .or_insert_with(|| Value::String(DEFAULT_MODE.to_owned()));
    table
        .entry("continue_prompt".to_owned())
        .or_insert_with(|| Value::String(DEFAULT_CONTINUE_PROMPT.to_owned()));
    table
        .entry("checks_profile".to_owned())
        .or_insert_with(|| Value::String(DEFAULT_CHECKS_PROFILE.to_owned()));

    workspaces.retain(|workspace| !workspace_path_matches(workspace, workspace_path));
    workspaces.insert(0, workspace);
    Ok(())
}

fn new_workspace_table() -> Value {
    Value::Table(toml::map::Map::new())
}

fn next_workspace_id(workspaces: &[Value], workspace_path: &Path) -> String {
    let base = workspace_id_base(workspace_path);
    if !workspace_id_exists(workspaces, &base) {
        return base;
    }
    format!("{base}-{}", workspace_hash(workspace_path))
}

fn workspace_id_base(workspace_path: &Path) -> String {
    let name = workspace_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let mut rendered = String::new();
    let mut previous_dash = false;
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            rendered.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            rendered.push('-');
            previous_dash = true;
        }
    }
    let trimmed = rendered.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn workspace_hash(workspace_path: &Path) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, render_path(workspace_path).as_bytes())
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

fn workspace_id_exists(workspaces: &[Value], workspace_id: &str) -> bool {
    workspaces.iter().any(|workspace| {
        workspace
            .as_table()
            .and_then(|table| table.get("id"))
            .and_then(Value::as_str)
            == Some(workspace_id)
    })
}

fn workspace_path_matches(workspace: &Value, workspace_path: &Path) -> bool {
    workspace
        .as_table()
        .and_then(|table| table.get("path"))
        .and_then(Value::as_str)
        .map(|path| {
            normalize_path_string(path) == normalize_path_string(&render_path(workspace_path))
        })
        .unwrap_or(false)
}

fn workspace_uses_placeholder_path(workspace: &Value) -> bool {
    workspace
        .as_table()
        .and_then(|table| table.get("path"))
        .and_then(Value::as_str)
        .map(|path| {
            normalize_path_string(path) == normalize_path_string(PLACEHOLDER_WORKSPACE_PATH)
        })
        .unwrap_or(false)
}

fn is_placeholder_path(path: &Path) -> bool {
    normalize_path_string(&render_path(path)) == normalize_path_string(PLACEHOLDER_WORKSPACE_PATH)
}

fn canonicalize_workspace(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("failed to resolve workspace path: {}", path.display()))?;
    if !canonical.is_dir() {
        bail!(
            "workspace path must be a directory: {}",
            canonical.display()
        );
    }
    Ok(canonical)
}

pub fn render_workspace_path(path: &Path) -> String {
    let raw = path.display().to_string();
    let without_extended_prefix = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        raw
    };
    without_extended_prefix.replace('\\', "/")
}

fn render_path(path: &Path) -> String {
    render_workspace_path(path)
}

fn normalize_path_string(path: &str) -> String {
    path.replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn upsert_workspace_creates_config_when_missing() {
        let dir = tempdir().expect("tempdir should exist");
        let config_path = dir.path().join("remotty").join("bridge.toml");
        let workspace = dir.path().join("Project One");
        fs::create_dir_all(&workspace).expect("workspace should exist");

        let result = upsert_workspace(&config_path, &workspace).expect("workspace should upsert");
        let written = fs::read_to_string(&config_path).expect("config should write");

        assert!(result.created_config);
        assert_eq!(result.workspace_id, "project-one");
        assert!(written.contains("project-one"));
        assert!(written.contains("Project One"));
        assert!(!written.contains(PLACEHOLDER_WORKSPACE_PATH));
    }

    #[test]
    fn upsert_workspace_does_not_duplicate_same_path() {
        let dir = tempdir().expect("tempdir should exist");
        let config_path = dir.path().join("bridge.toml");
        let workspace = dir.path().join("app");
        fs::create_dir_all(&workspace).expect("workspace should exist");

        upsert_workspace(&config_path, &workspace).expect("first upsert");
        upsert_workspace(&config_path, &workspace).expect("second upsert");
        let config: Value = fs::read_to_string(&config_path)
            .expect("config should read")
            .parse()
            .expect("config should parse");
        let workspaces = config
            .get("workspaces")
            .and_then(Value::as_array)
            .expect("workspaces should exist");

        assert_eq!(workspaces.len(), 1);
    }

    #[test]
    fn upsert_workspace_adds_hash_when_id_collides() {
        let dir = tempdir().expect("tempdir should exist");
        let config_path = dir.path().join("bridge.toml");
        let first = dir.path().join("one").join("app");
        let second = dir.path().join("two").join("app");
        fs::create_dir_all(&first).expect("first workspace should exist");
        fs::create_dir_all(&second).expect("second workspace should exist");

        let first_result = upsert_workspace(&config_path, &first).expect("first upsert");
        let second_result = upsert_workspace(&config_path, &second).expect("second upsert");

        assert_eq!(first_result.workspace_id, "app");
        assert!(second_result.workspace_id.starts_with("app-"));
        assert_ne!(first_result.workspace_id, second_result.workspace_id);
    }
}
