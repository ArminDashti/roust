//! Download Iran aggregated IP delegation data and write text lists.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Raw JSON (GitHub `blob` URLs serve HTML; use raw.githubusercontent.com).
const IR_AGGREGATED_JSON_URL: &str =
    "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ir/aggregated.json";

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

/// Run `roust update`: fetch JSON, replace ipv4/ipv6 list files in `out_dir`.
pub fn run(out_dir: &Path) -> Result<()> {
    let body = ureq::get(IR_AGGREGATED_JSON_URL)
        .call()
        .context("HTTP GET for Iran aggregated IP JSON")?
        .into_string()
        .context("read Iran aggregated IP JSON response body")?;

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
        let mut f = fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
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
}
