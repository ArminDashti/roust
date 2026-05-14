//! Download IP delegation JSON payloads and write companion text lists.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Raw JSON (GitHub `blob` URLs serve HTML; use raw.githubusercontent.com).
pub const IR_AGGREGATED_JSON_URL: &str =
    "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ir/aggregated.json";

/// Repository `private_ips.json` (array of CIDR strings); override with `ROUST_PRIVATE_IPS_JSON_URL`.
pub const PRIVATE_IPS_JSON_URL: &str =
    "https://raw.githubusercontent.com/ArminDashti/roust/main/private_ips.json";

#[derive(Debug, Deserialize)]
struct AggregatedPayload {
    prefixes: PrefixesBlock,
}

#[derive(Debug, Deserialize)]
struct PrefixesBlock {
    #[serde(default)]
    ipv4: Vec<String>,
    #[serde(default)]
    ipv6: Vec<String>,
}

fn http_get_string(url: &str) -> Result<String> {
    ureq::get(url)
        .call()
        .with_context(|| format!("HTTP GET {url}"))?
        .into_string()
        .with_context(|| format!("read response body from {url}"))
}

/// Run `roust update`: fetch JSON, write `iran_aggregated.json`, replace ipv4/ipv6 list files in `out_dir`.
pub fn run(out_dir: &Path) -> Result<()> {
    let url = std::env::var("ROUST_IR_AGGREGATED_JSON_URL")
        .unwrap_or_else(|_| IR_AGGREGATED_JSON_URL.to_string());
    let body = http_get_string(&url).context("Iran aggregated IP JSON")?;

    let ir_json_path = out_dir.join("iran_aggregated.json");
    fs::write(&ir_json_path, &body).with_context(|| format!("write {}", ir_json_path.display()))?;

    let payload: AggregatedPayload =
        serde_json::from_str(&body).context("parse Iran aggregated IP JSON")?;

    let ipv4_cidr: Vec<String> = payload.prefixes.ipv4;
    let ipv6_cidr: Vec<String> = payload.prefixes.ipv6;

    let ipv4_plain: Vec<String> = ipv4_cidr.iter().map(|s| strip_subnet(s)).collect();
    let ipv6_plain: Vec<String> = ipv6_cidr.iter().map(|s| strip_subnet(s)).collect();

    let files: [(&str, &[String]); 4] = [
        ("ipv4-cidr.txt", &ipv4_cidr),
        ("ipv6-cidr.txt", &ipv6_cidr),
        ("ipv4.txt", &ipv4_plain),
        ("ipv6.txt", &ipv6_plain),
    ];

    for (name, _) in &files {
        let path = out_dir.join(name);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
    }

    for (name, lines) in &files {
        let path = out_dir.join(name);
        let mut f =
            fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
        for line in *lines {
            writeln!(f, "{line}").with_context(|| format!("write {}", path.display()))?;
        }
    }

    log::info!(
        "wrote {} IPv4 and {} IPv6 prefixes to {}",
        ipv4_cidr.len(),
        ipv6_cidr.len(),
        out_dir.display()
    );

    Ok(())
}

/// Download `private_ips.json`, save it under `out_dir`, and emit `private_ips-cidr.txt` / `private_ips.txt`.
pub fn run_private_ips(out_dir: &Path) -> Result<()> {
    let url = std::env::var("ROUST_PRIVATE_IPS_JSON_URL")
        .unwrap_or_else(|_| PRIVATE_IPS_JSON_URL.to_string());
    let body = http_get_string(&url).context("private_ips JSON")?;

    let json_path = out_dir.join("private_ips.json");
    fs::write(&json_path, &body).with_context(|| format!("write {}", json_path.display()))?;

    let cidrs: Vec<String> =
        serde_json::from_str(&body).context("parse private_ips.json (expected JSON array of strings)")?;

    let plain: Vec<String> = cidrs.iter().map(|s| strip_subnet(s)).collect();

    let files: [(&str, &[String]); 2] =
        [("private_ips-cidr.txt", &cidrs), ("private_ips.txt", &plain)];

    for (name, _) in &files {
        let path = out_dir.join(name);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
    }

    for (name, lines) in &files {
        let path = out_dir.join(name);
        let mut f =
            fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
        for line in *lines {
            writeln!(f, "{line}").with_context(|| format!("write {}", path.display()))?;
        }
    }

    log::info!(
        "wrote {} private IP CIDR rows under {}",
        cidrs.len(),
        out_dir.display()
    );

    Ok(())
}

fn strip_subnet(cidr: &str) -> String {
    cidr.split('/').next().unwrap_or(cidr).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_subnet_ipv4() {
        assert_eq!(strip_subnet("2.57.3.0/24"), "2.57.3.0");
        assert_eq!(strip_subnet("2.144.0.0/14"), "2.144.0.0");
    }

    #[test]
    fn strip_subnet_ipv6() {
        assert_eq!(
            strip_subnet("2001:678:b0::/46"),
            "2001:678:b0::"
        );
    }

    #[test]
    fn parse_sample_payload() {
        let j = r#"{
            "country": "Iran",
            "countryCode": "IR",
            "prefixes": {
                "ipv4": ["1.2.3.0/24"],
                "ipv6": ["2001:db8::/32"]
            }
        }"#;
        let p: AggregatedPayload = serde_json::from_str(j).unwrap();
        assert_eq!(p.prefixes.ipv4, vec!["1.2.3.0/24"]);
        assert_eq!(p.prefixes.ipv6, vec!["2001:db8::/32"]);
    }

    #[test]
    fn parse_private_ips_payload() {
        let j = r#"["10.0.0.0/8","172.16.0.0/12"]"#;
        let v: Vec<String> = serde_json::from_str(j).unwrap();
        assert_eq!(v.len(), 2);
    }
}
