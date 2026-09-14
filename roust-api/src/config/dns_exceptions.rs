//! Per-hostname DNS server overrides (`dns-exceptions.json`), separate from routes.
//! Applied via Windows Name Resolution Policy Table (NRPT), Comment=`Roust`.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const NRPT_COMMENT: &str = "Roust";

/// One namespace → DNS server override as stored in `dns-exceptions.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsException {
    /// Hostname or DNS namespace (e.g. `example.com` or `.example.com`).
    pub namespace: String,
    #[serde(rename = "dns-server")]
    pub dns_server: String,
}

impl DnsException {
    pub fn validate(&self) -> Result<()> {
        validate_namespace(&self.namespace)?;
        validate_dns_server(&self.dns_server)?;
        Ok(())
    }

    pub fn identity_key(&self) -> String {
        normalize_namespace(&self.namespace)
    }

    pub fn label(&self) -> String {
        format!(
            "{} → {}",
            self.namespace.trim(),
            self.dns_server.trim()
        )
    }

    /// PowerShell args for `Add-DnsClientNrptRule` (excluding the cmdlet name).
    pub fn nrpt_add_args(&self) -> Result<Vec<String>> {
        self.validate()?;
        Ok(vec![
            "-Namespace".into(),
            nrpt_namespace(&self.namespace),
            "-NameServers".into(),
            self.dns_server.trim().to_string(),
            "-Comment".into(),
            NRPT_COMMENT.into(),
        ])
    }
}

fn validate_dns_server(value: &str) -> Result<Ipv4Addr> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("dns-server must not be empty"));
    }
    trimmed
        .parse::<Ipv4Addr>()
        .map_err(|_| anyhow!("dns-server \"{trimmed}\" must be an IPv4 address"))
}

fn validate_namespace(value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("namespace must not be empty"));
    }
    if trimmed.contains("://") || trimmed.contains('/') || trimmed.contains('\\') {
        return Err(anyhow!(
            "namespace \"{trimmed}\" must not include a URL scheme or path"
        ));
    }
    if trimmed.contains(':') {
        return Err(anyhow!(
            "namespace \"{trimmed}\" must not include a port"
        ));
    }
    if trimmed.contains('*') {
        return Err(anyhow!(
            "namespace \"{trimmed}\" must not include wildcards"
        ));
    }
    let bare = trimmed.trim_start_matches('.');
    if bare.is_empty() {
        return Err(anyhow!("namespace \"{trimmed}\" is malformed"));
    }
    if !bare
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err(anyhow!(
            "namespace \"{trimmed}\" contains invalid characters"
        ));
    }
    Ok(())
}

fn normalize_namespace(value: &str) -> String {
    value.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn nrpt_namespace(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('.') {
        trimmed.to_string()
    } else {
        // Suffix match for the domain and its subdomains.
        format!(".{trimmed}")
    }
}

/// In-memory store for `dns-exceptions.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsExceptionStore {
    pub exceptions: Vec<DnsException>,
}

impl DnsExceptionStore {
    pub fn new() -> Self {
        Self {
            exceptions: vec![],
        }
    }

    pub fn path_beside(routes_path: &Path) -> PathBuf {
        routes_path.with_file_name("dns-exceptions.json")
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
        let exceptions: Vec<DnsException> = serde_json::from_str(trimmed).map_err(|e| {
            anyhow!(
                "invalid dns-exceptions JSON (expected [{{\"namespace\":\"...\",\"dns-server\":\"...\"}}]): {e}"
            )
        })?;
        validate_exceptions(&exceptions)?;
        Ok(Self { exceptions })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.exceptions)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn get_exceptions(&self) -> &[DnsException] {
        &self.exceptions
    }

    pub fn add(&mut self, exception: DnsException) -> Result<()> {
        exception.validate()?;
        ensure_unique(&exception, &self.exceptions, None)?;
        self.exceptions.push(normalize_exception(exception));
        Ok(())
    }

    pub fn replace_at(&mut self, index: usize, exception: DnsException) -> Result<()> {
        if index >= self.exceptions.len() {
            return Err(anyhow!("dns exception index {index} not found"));
        }
        exception.validate()?;
        ensure_unique(&exception, &self.exceptions, Some(index))?;
        self.exceptions[index] = normalize_exception(exception);
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.exceptions.len() {
            return false;
        }
        self.exceptions.remove(index);
        true
    }
}

fn normalize_exception(mut exception: DnsException) -> DnsException {
    exception.namespace = exception.namespace.trim().to_string();
    exception.dns_server = exception.dns_server.trim().to_string();
    exception
}

fn validate_exceptions(exceptions: &[DnsException]) -> Result<()> {
    for exception in exceptions {
        exception.validate()?;
    }
    let mut seen = Vec::new();
    for exception in exceptions {
        let key = exception.identity_key();
        if seen.iter().any(|k| k == &key) {
            return Err(anyhow!(
                "duplicate dns exception namespace \"{key}\""
            ));
        }
        seen.push(key);
    }
    Ok(())
}

fn ensure_unique(
    exception: &DnsException,
    existing: &[DnsException],
    skip_index: Option<usize>,
) -> Result<()> {
    let key = exception.identity_key();
    for (i, other) in existing.iter().enumerate() {
        if Some(i) == skip_index {
            continue;
        }
        if other.identity_key() == key {
            return Err(anyhow!(
                "duplicate dns exception namespace \"{key}\""
            ));
        }
    }
    Ok(())
}

/// Replace all NRPT rules with Comment=`Roust` to match `exceptions`.
pub fn apply_nrpt(exceptions: &[DnsException]) -> Result<()> {
    clear_roust_nrpt()?;
    for exception in exceptions {
        let args = exception.nrpt_add_args()?;
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Add-DnsClientNrptRule {}",
                args.iter()
                    .map(|a| {
                        if a.starts_with('-') {
                            a.clone()
                        } else {
                            format!("'{}'", a.replace('\'', "''"))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ]);
        let output = cmd
            .output()
            .map_err(|e| anyhow!("failed to run Add-DnsClientNrptRule: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow!(
                "Add-DnsClientNrptRule failed for {}: {stderr}{stdout}",
                exception.label()
            ));
        }
        log::info!("NRPT applied: {}", exception.label());
    }
    Ok(())
}

/// Remove every NRPT rule owned by Roust (Comment=`Roust`).
pub fn clear_roust_nrpt() -> Result<()> {
    let script = format!(
        "Get-DnsClientNrptRule | Where-Object {{ $_.Comment -eq '{NRPT_COMMENT}' }} | ForEach-Object {{ Remove-DnsClientNrptRule -Name $_.Name -Force }}"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| anyhow!("failed to clear Roust NRPT rules: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Empty / no rules is fine; treat real failures only.
        if !stderr.trim().is_empty() && !stderr.contains("ObjectNotFound") {
            return Err(anyhow!(
                "Remove-DnsClientNrptRule failed: {stderr}{stdout}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DnsException {
        DnsException {
            namespace: "example.com".into(),
            dns_server: "8.8.8.8".into(),
        }
    }

    #[test]
    fn test_load_empty_and_valid() {
        let store = DnsExceptionStore::from_json_str("[]").unwrap();
        assert!(store.exceptions.is_empty());

        let json = r#"[{"namespace":"cdn.example.com","dns-server":"1.1.1.1"}]"#;
        let store = DnsExceptionStore::from_json_str(json).unwrap();
        assert_eq!(store.exceptions.len(), 1);
        assert_eq!(store.exceptions[0].dns_server, "1.1.1.1");
    }

    #[test]
    fn test_reject_bad_dns_server() {
        let mut ex = sample();
        ex.dns_server = "not-an-ip".into();
        assert!(ex.validate().is_err());
    }

    #[test]
    fn test_reject_duplicate_namespace() {
        let json = r#"[
            {"namespace":"example.com","dns-server":"8.8.8.8"},
            {"namespace":".Example.COM","dns-server":"1.1.1.1"}
        ]"#;
        let err = DnsExceptionStore::from_json_str(json).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn test_nrpt_add_args() {
        let args = sample().nrpt_add_args().unwrap();
        assert_eq!(
            args,
            vec![
                String::from("-Namespace"),
                String::from(".example.com"),
                String::from("-NameServers"),
                String::from("8.8.8.8"),
                String::from("-Comment"),
                String::from("Roust"),
            ]
        );
    }

    #[test]
    fn test_path_beside_routes() {
        let routes = PathBuf::from(r"C:\ProgramData\roust\routes.json");
        assert_eq!(
            DnsExceptionStore::path_beside(&routes),
            PathBuf::from(r"C:\ProgramData\roust\dns-exceptions.json")
        );
    }

    #[test]
    fn test_add_replace_remove() {
        let mut store = DnsExceptionStore::new();
        store.add(sample()).unwrap();
        assert_eq!(store.exceptions.len(), 1);

        let mut updated = sample();
        updated.dns_server = "1.1.1.1".into();
        store.replace_at(0, updated).unwrap();
        assert_eq!(store.exceptions[0].dns_server, "1.1.1.1");

        assert!(store.remove_at(0));
        assert!(store.exceptions.is_empty());
    }
}
