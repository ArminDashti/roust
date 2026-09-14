//! Per-hostname → IPv4 overrides (`host-overrides.json`), separate from routes.
//! Applied via a marked block in the Windows hosts file (checked before DNS).

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{validate_hostname, validate_ipv4};

pub const HOSTS_BEGIN: &str = "# ROUST-HOST-BEGIN";
pub const HOSTS_END: &str = "# ROUST-HOST-END";

/// One hostname → IPv4 override as stored in `host-overrides.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostOverride {
    pub hostname: String,
    pub ip: String,
}

impl HostOverride {
    pub fn validate(&self) -> Result<()> {
        validate_hostname(&self.hostname)?;
        let ip = validate_ipv4(&self.ip)?;
        if ip.is_unspecified() || ip.is_loopback() || ip.is_multicast() {
            return Err(anyhow!(
                "ip \"{}\" must be a unicast non-loopback IPv4 address",
                self.ip.trim()
            ));
        }
        Ok(())
    }

    pub fn identity_key(&self) -> String {
        self.hostname.trim().trim_end_matches('.').to_ascii_lowercase()
    }

    pub fn label(&self) -> String {
        format!("{} → {}", self.hostname.trim(), self.ip.trim())
    }

    /// One hosts-file line: `ip hostname`.
    pub fn hosts_line(&self) -> Result<String> {
        self.validate()?;
        let host = validate_hostname(&self.hostname)?;
        let ip = validate_ipv4(&self.ip)?;
        Ok(format!("{ip} {host}"))
    }
}

/// In-memory store for `host-overrides.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostOverrideStore {
    pub overrides: Vec<HostOverride>,
}

impl HostOverrideStore {
    pub fn new() -> Self {
        Self {
            overrides: vec![],
        }
    }

    pub fn path_beside(routes_path: &Path) -> PathBuf {
        routes_path.with_file_name("host-overrides.json")
    }

    pub fn default_path() -> PathBuf {
        Self::path_beside(&super::Config::default_config_path())
    }

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
        let overrides: Vec<HostOverride> = serde_json::from_str(trimmed).map_err(|e| {
            anyhow!(
                "invalid host-overrides JSON (expected [{{\"hostname\":\"...\",\"ip\":\"...\"}}]): {e}"
            )
        })?;
        validate_overrides(&overrides)?;
        Ok(Self { overrides })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.overrides)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn get_overrides(&self) -> &[HostOverride] {
        &self.overrides
    }

    pub fn add(&mut self, override_: HostOverride) -> Result<()> {
        override_.validate()?;
        ensure_unique(&override_, &self.overrides, None)?;
        self.overrides.push(normalize_override(override_));
        Ok(())
    }

    pub fn replace_at(&mut self, index: usize, override_: HostOverride) -> Result<()> {
        if index >= self.overrides.len() {
            return Err(anyhow!("host override index {index} not found"));
        }
        override_.validate()?;
        ensure_unique(&override_, &self.overrides, Some(index))?;
        self.overrides[index] = normalize_override(override_);
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.overrides.len() {
            return false;
        }
        self.overrides.remove(index);
        true
    }
}

fn normalize_override(mut override_: HostOverride) -> HostOverride {
    override_.hostname = override_.hostname.trim().trim_end_matches('.').to_ascii_lowercase();
    override_.ip = override_.ip.trim().to_string();
    override_
}

fn validate_overrides(overrides: &[HostOverride]) -> Result<()> {
    for override_ in overrides {
        override_.validate()?;
    }
    let mut seen = Vec::new();
    for override_ in overrides {
        let key = override_.identity_key();
        if seen.iter().any(|k| k == &key) {
            return Err(anyhow!("duplicate host override hostname \"{key}\""));
        }
        seen.push(key);
    }
    Ok(())
}

fn ensure_unique(
    override_: &HostOverride,
    existing: &[HostOverride],
    skip_index: Option<usize>,
) -> Result<()> {
    let key = override_.identity_key();
    for (i, other) in existing.iter().enumerate() {
        if Some(i) == skip_index {
            continue;
        }
        if other.identity_key() == key {
            return Err(anyhow!("duplicate host override hostname \"{key}\""));
        }
    }
    Ok(())
}

/// Default Windows hosts path.
pub fn default_hosts_path() -> PathBuf {
    PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
}

/// Rewrite `content`, replacing (or appending) the Roust-managed hosts block.
pub fn rewrite_hosts_content(content: &str, overrides: &[HostOverride]) -> Result<String> {
    for override_ in overrides {
        override_.validate()?;
    }

    let mut out = String::new();
    let mut in_block = false;
    let mut saw_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == HOSTS_BEGIN {
            in_block = true;
            saw_block = true;
            continue;
        }
        if trimmed == HOSTS_END {
            in_block = false;
            continue;
        }
        if in_block {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    if overrides.is_empty() {
        // Drop trailing blank lines introduced by stripping the block only when empty.
        while out.ends_with("\n\n") {
            out.pop();
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        return Ok(out);
    }

    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !saw_block {
        // Separate from prior content with a blank line when appending.
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
    }

    out.push_str(HOSTS_BEGIN);
    out.push('\n');
    for override_ in overrides {
        out.push_str(&override_.hosts_line()?);
        out.push('\n');
    }
    out.push_str(HOSTS_END);
    out.push('\n');
    Ok(out)
}

fn flush_dns_cache() {
    let _ = Command::new("ipconfig").arg("/flushdns").output();
}

/// Replace the Roust hosts block to match `overrides`, then flush DNS cache.
pub fn apply_hosts(overrides: &[HostOverride]) -> Result<()> {
    apply_hosts_at(&default_hosts_path(), overrides)
}

fn hosts_security_lock_hint() -> &'static str {
    // ponytail: path probe only — upgrade: query filter-driver / AV product list via WMI.
    if Path::new(r"C:\Program Files (x86)\Kaspersky Lab").is_dir()
        || Path::new(r"C:\Program Files\Kaspersky Lab").is_dir()
    {
        " Kaspersky Endpoint Security is installed and commonly locks the hosts file even for Administrators and SYSTEM. Allow hosts-file modification for roust-api.exe and the Roust service (or ask your KSC admin), then retry."
    } else {
        " A security product may be locking the hosts file; allow modification for roust-api.exe / Local System, then retry."
    }
}

fn map_hosts_io_error(path: &Path, err: std::io::Error, action: &str) -> anyhow::Error {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        anyhow!(
            "failed to {action} hosts file {}: {}.{}",
            path.display(),
            err,
            hosts_security_lock_hint()
        )
    } else {
        anyhow!("failed to {action} hosts file {}: {err}", path.display())
    }
}

pub fn apply_hosts_at(hosts_path: &Path, overrides: &[HostOverride]) -> Result<()> {
    let existing = if hosts_path.exists() {
        fs::read_to_string(hosts_path).map_err(|e| map_hosts_io_error(hosts_path, e, "read"))?
    } else {
        String::new()
    };
    let rewritten = rewrite_hosts_content(&existing, overrides)?;
    fs::write(hosts_path, &rewritten).map_err(|e| map_hosts_io_error(hosts_path, e, "write"))?;
    for override_ in overrides {
        log::info!("hosts applied: {}", override_.label());
    }
    flush_dns_cache();
    Ok(())
}

/// Remove the Roust-managed hosts block.
pub fn clear_roust_hosts() -> Result<()> {
    apply_hosts(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample() -> HostOverride {
        HostOverride {
            hostname: "example.com".into(),
            ip: "203.0.113.10".into(),
        }
    }

    #[test]
    fn test_load_empty_and_valid() {
        let store = HostOverrideStore::from_json_str("[]").unwrap();
        assert!(store.overrides.is_empty());

        let json = r#"[{"hostname":"cdn.example.com","ip":"1.2.3.4"}]"#;
        let store = HostOverrideStore::from_json_str(json).unwrap();
        assert_eq!(store.overrides.len(), 1);
        assert_eq!(store.overrides[0].ip, "1.2.3.4");
    }

    #[test]
    fn test_reject_loopback_ip() {
        let mut o = sample();
        o.ip = "127.0.0.1".into();
        assert!(o.validate().is_err());
    }

    #[test]
    fn test_reject_duplicate_hostname() {
        let json = r#"[
            {"hostname":"example.com","ip":"1.2.3.4"},
            {"hostname":"Example.COM.","ip":"5.6.7.8"}
        ]"#;
        let err = HostOverrideStore::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn test_path_beside_routes() {
        let routes = PathBuf::from(r"C:\ProgramData\roust\routes.json");
        assert_eq!(
            HostOverrideStore::path_beside(&routes),
            PathBuf::from(r"C:\ProgramData\roust\host-overrides.json")
        );
    }

    #[test]
    fn test_add_replace_remove() {
        let mut store = HostOverrideStore::new();
        store.add(sample()).unwrap();
        assert_eq!(store.overrides.len(), 1);

        let mut updated = sample();
        updated.ip = "198.51.100.1".into();
        store.replace_at(0, updated).unwrap();
        assert_eq!(store.overrides[0].ip, "198.51.100.1");

        assert!(store.remove_at(0));
        assert!(store.overrides.is_empty());
    }

    #[test]
    fn test_rewrite_hosts_round_trip() {
        let base = "127.0.0.1 localhost\n::1 localhost\n";
        let with_block = rewrite_hosts_content(base, &[sample()]).unwrap();
        assert!(with_block.contains(HOSTS_BEGIN));
        assert!(with_block.contains("203.0.113.10 example.com"));
        assert!(with_block.contains(HOSTS_END));
        assert!(with_block.contains("127.0.0.1 localhost"));

        let updated = HostOverride {
            hostname: "example.com".into(),
            ip: "198.51.100.9".into(),
        };
        let replaced = rewrite_hosts_content(&with_block, &[updated]).unwrap();
        assert!(replaced.contains("198.51.100.9 example.com"));
        assert!(!replaced.contains("203.0.113.10"));
        assert_eq!(replaced.matches(HOSTS_BEGIN).count(), 1);

        let cleared = rewrite_hosts_content(&replaced, &[]).unwrap();
        assert!(!cleared.contains(HOSTS_BEGIN));
        assert!(!cleared.contains(HOSTS_END));
        assert!(cleared.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn test_apply_hosts_at_temp_file() {
        let dir = std::env::temp_dir().join(format!(
            "roust-hosts-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("hosts");
        fs::write(&path, "127.0.0.1 localhost\n").unwrap();

        apply_hosts_at(&path, &[sample()]).unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("203.0.113.10 example.com"));

        apply_hosts_at(&path, &[]).unwrap();
        let cleared = fs::read_to_string(&path).unwrap();
        assert!(!cleared.contains(HOSTS_BEGIN));
        assert!(cleared.contains("127.0.0.1 localhost"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_hosts_write_permission_message() {
        let path = Path::new(r"C:\Windows\System32\drivers\etc\hosts");
        let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access is denied.");
        let msg = map_hosts_io_error(path, err, "write").to_string();
        assert!(msg.contains("failed to write hosts file"));
        assert!(msg.contains("Access is denied"));
        assert!(
            msg.contains("security product") || msg.contains("Kaspersky"),
            "expected lock hint, got: {msg}"
        );
    }
}
