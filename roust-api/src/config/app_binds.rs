//! Per-app NIC binding config (`app-binds.json`), separate from destination routes.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// One app → NIC binding as stored in `app-binds.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppBind {
    /// Full executable path when known (preferred for WFP app ID).
    #[serde(rename = "exe-path", default, skip_serializing_if = "Option::is_none")]
    pub exe_path: Option<String>,
    /// Process image name (e.g. `AppExample.exe`); used when path is missing.
    #[serde(rename = "image-name", default, skip_serializing_if = "Option::is_none")]
    pub image_name: Option<String>,
    /// NIC alias (friendly / display / internal name), same matching as routes.
    pub nic: String,
}

impl AppBind {
    pub fn validate(&self) -> Result<()> {
        let path = self.exe_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let image = self
            .image_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if path.is_none() && image.is_none() {
            return Err(anyhow!(
                "app bind requires exe-path and/or image-name"
            ));
        }
        if self.nic.trim().is_empty() {
            return Err(anyhow!("app bind nic must not be empty"));
        }
        Ok(())
    }

    /// Identity key for duplicate detection: normalized path, else lowercase image name.
    pub fn identity_key(&self) -> String {
        if let Some(path) = self
            .exe_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return normalize_exe_path(path);
        }
        self.image_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
            .to_ascii_lowercase()
    }

    pub fn label(&self) -> String {
        let app = self
            .exe_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.image_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("?");
        format!("{app} → {}", self.nic.trim())
    }
}

fn normalize_exe_path(path: &str) -> String {
    path.replace('/', "\\").to_ascii_lowercase()
}

/// Resolved health of a bind against live adapters / app ID resolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AppBindStatus {
    Healthy,
    NicDown,
    Unresolved,
}

/// In-memory store for `app-binds.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppBindStore {
    pub binds: Vec<AppBind>,
}

impl AppBindStore {
    pub fn new() -> Self {
        Self { binds: vec![] }
    }

    /// Path to `app-binds.json` beside a `routes.json` path.
    pub fn path_beside(routes_path: &Path) -> PathBuf {
        routes_path.with_file_name("app-binds.json")
    }

    pub fn default_path() -> PathBuf {
        Self::path_beside(&super::Config::default_config_path())
    }

    /// Load from disk; missing file yields an empty store.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = fs::read_to_string(path)?;
        Self::from_json_str(&contents)
    }

    pub fn from_json_str(contents: &str) -> Result<Self> {
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Ok(Self::new());
        }
        let binds: Vec<AppBind> = serde_json::from_str(trimmed).map_err(|e| {
            anyhow!(
                "invalid app-binds JSON (expected [{{\"exe-path\":\"...\",\"image-name\":\"...\",\"nic\":\"...\"}}]): {e}"
            )
        })?;
        validate_binds(&binds)?;
        Ok(Self { binds })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.binds)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn get_binds(&self) -> &[AppBind] {
        &self.binds
    }

    pub fn add(&mut self, bind: AppBind) -> Result<()> {
        bind.validate()?;
        ensure_unique_identity(&bind, &self.binds, None)?;
        self.binds.push(normalize_bind(bind));
        Ok(())
    }

    pub fn replace_at(&mut self, index: usize, bind: AppBind) -> Result<()> {
        if index >= self.binds.len() {
            return Err(anyhow!("app bind index {index} not found"));
        }
        bind.validate()?;
        ensure_unique_identity(&bind, &self.binds, Some(index))?;
        self.binds[index] = normalize_bind(bind);
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.binds.len() {
            return false;
        }
        self.binds.remove(index);
        true
    }
}

fn normalize_bind(mut bind: AppBind) -> AppBind {
    bind.exe_path = bind
        .exe_path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    bind.image_name = bind
        .image_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    bind.nic = bind.nic.trim().to_string();
    bind
}

fn validate_binds(binds: &[AppBind]) -> Result<()> {
    for bind in binds {
        bind.validate()?;
    }
    let mut seen = Vec::new();
    for bind in binds {
        let key = bind.identity_key();
        if seen.iter().any(|k| k == &key) {
            return Err(anyhow!(
                "duplicate app bind identity \"{key}\" (use a unique exe-path or image-name)"
            ));
        }
        seen.push(key);
    }
    Ok(())
}

fn ensure_unique_identity(
    bind: &AppBind,
    existing: &[AppBind],
    skip_index: Option<usize>,
) -> Result<()> {
    let key = bind.identity_key();
    for (i, other) in existing.iter().enumerate() {
        if Some(i) == skip_index {
            continue;
        }
        if other.identity_key() == key {
            return Err(anyhow!(
                "duplicate app bind identity \"{key}\" (use a unique exe-path or image-name)"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppBind {
        AppBind {
            exe_path: Some(r"C:\Program Files\AppExample\AppExample.exe".into()),
            image_name: Some("AppExample.exe".into()),
            nic: "Realtek".into(),
        }
    }

    #[test]
    fn test_load_empty_and_valid() {
        let store = AppBindStore::from_json_str("[]").unwrap();
        assert!(store.binds.is_empty());

        let json = r#"[{
            "exe-path": "C:\\Apps\\a.exe",
            "image-name": "a.exe",
            "nic": "Ethernet"
        }]"#;
        let store = AppBindStore::from_json_str(json).unwrap();
        assert_eq!(store.binds.len(), 1);
        assert_eq!(store.binds[0].nic, "Ethernet");
    }

    #[test]
    fn test_reject_missing_identity() {
        let json = r#"[{"nic":"Ethernet"}]"#;
        let err = AppBindStore::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("exe-path"));
    }

    #[test]
    fn test_reject_empty_nic() {
        let mut bind = sample();
        bind.nic = "  ".into();
        assert!(bind.validate().is_err());
    }

    #[test]
    fn test_reject_duplicate_path() {
        let json = r#"[
            {"exe-path":"C:\\a.exe","nic":"Eth1"},
            {"exe-path":"c:/a.exe","nic":"Eth2"}
        ]"#;
        let err = AppBindStore::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn test_reject_duplicate_image_name() {
        let json = r#"[
            {"image-name":"App.exe","nic":"Eth1"},
            {"image-name":"app.EXE","nic":"Eth2"}
        ]"#;
        let err = AppBindStore::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn test_add_replace_remove() {
        let mut store = AppBindStore::new();
        store.add(sample()).unwrap();
        assert_eq!(store.binds.len(), 1);

        let mut updated = sample();
        updated.nic = "Wi-Fi".into();
        store.replace_at(0, updated).unwrap();
        assert_eq!(store.binds[0].nic, "Wi-Fi");

        assert!(store.remove_at(0));
        assert!(store.binds.is_empty());
        assert!(!store.remove_at(0));
    }

    #[test]
    fn test_path_beside_routes() {
        let routes = PathBuf::from(r"C:\ProgramData\roust\routes.json");
        assert_eq!(
            AppBindStore::path_beside(&routes),
            PathBuf::from(r"C:\ProgramData\roust\app-binds.json")
        );
    }

    #[test]
    fn test_image_name_only_ok() {
        let bind = AppBind {
            exe_path: None,
            image_name: Some("chrome.exe".into()),
            nic: "Ethernet".into(),
        };
        bind.validate().unwrap();
        assert_eq!(bind.identity_key(), "chrome.exe");
    }
}
