use anyhow::{Context, Result}; // Import anyhow helpers for clearer error messages around I/O and HTTP
use serde::Deserialize; // Import the Deserialize trait so serde can build structs from JSON text
use std::fs; // Import filesystem helpers for reading and writing list files on disk
use std::io::Write; // Import the Write trait so we can append lines with writeln
use std::path::Path; // Import the Path type for functions that accept output directories

pub const IR_AGGREGATED_JSON_URL: &str = "https://raw.githubusercontent.com/ipverse/country-ip-blocks/master/country/ir/aggregated.json"; // Default raw.githubusercontent URL for Iran aggregated IP JSON

pub const PRIVATE_IPS_JSON_URL: &str = "https://raw.githubusercontent.com/ArminDashti/roust/main/private_ips.json"; // Default raw.githubusercontent URL for the repository private IP JSON list

#[derive(Debug, Deserialize)] // Derive Debug printing and serde deserialization for aggregated JSON payloads
struct AggregatedPayload {
    prefixes: PrefixesBlock, // Hold the nested prefixes object that contains IP string vectors
} // Close the AggregatedPayload struct

#[derive(Debug, Deserialize)] // Derive Debug printing and serde deserialization for the prefixes object
struct PrefixesBlock {
    #[serde(default)] // Treat missing ipv4 arrays as empty instead of failing deserialization
    ipv4: Vec<String>, // Store every IPv4 CIDR string provided by the remote JSON payload
    #[serde(default)] // Treat missing ipv6 arrays as empty instead of failing deserialization
    ipv6: Vec<String>, // Store every IPv6 CIDR string provided by the remote JSON payload
} // Close the PrefixesBlock struct

fn http_get_string(url: &str) -> Result<String> {
    ureq::get(url) // Start a blocking GET request for the requested URL string
        .call() // Perform the network request and obtain a response handle
        .with_context(|| format!("HTTP GET {url}"))? // Attach the URL to errors that happen before a body exists
        .into_string() // Read the entire response body into a single UTF-8 String value
        .with_context(|| format!("read response body from {url}")) // Attach the URL to errors while reading bytes and return the final Result unchanged
} // Close the http_get_string function

pub fn run(out_dir: &Path) -> Result<()> {
    let url = std::env::var("ROUST_IR_AGGREGATED_JSON_URL").unwrap_or_else(|_| IR_AGGREGATED_JSON_URL.to_string()); // Allow operators to override the Iran JSON download URL via environment
    let body = http_get_string(&url).context("Iran aggregated IP JSON")?; // Download the JSON document as text for parsing and archival

    let ir_json_path = out_dir.join("iran_aggregated.json"); // Build the output path for the raw JSON snapshot file
    fs::write(&ir_json_path, &body).with_context(|| format!("write {}", ir_json_path.display()))?; // Persist the exact JSON bytes for auditing and reuse

    let payload: AggregatedPayload = serde_json::from_str(&body).context("parse Iran aggregated IP JSON")?; // Parse the JSON text into strongly typed Rust structures

    let ipv4_cidr: Vec<String> = payload.prefixes.ipv4; // Copy the IPv4 CIDR list out of the parsed payload for downstream processing
    let ipv6_cidr: Vec<String> = payload.prefixes.ipv6; // Copy the IPv6 CIDR list out of the parsed payload for downstream processing

    let ipv4_plain: Vec<String> = ipv4_cidr.iter().map(|s| strip_subnet(s)).collect(); // Build host-only IPv4 strings by stripping any slash suffix from each CIDR
    let ipv6_plain: Vec<String> = ipv6_cidr.iter().map(|s| strip_subnet(s)).collect(); // Build host-only IPv6 strings by stripping any slash suffix from each CIDR

    let files: [(&str, &[String]); 4] = [
        ("ipv4_cidr.txt", &ipv4_cidr), // Map the first output filename to the IPv4 CIDR vector slice
        ("ipv6_cidr.txt", &ipv6_cidr), // Map the second output filename to the IPv6 CIDR vector slice
        ("ipv4.txt", &ipv4_plain), // Map the third output filename to the plain IPv4 host strings slice
        ("ipv6.txt", &ipv6_plain), // Map the fourth output filename to the plain IPv6 host strings slice
    ]; // Close the static table that drives file generation

    for (name, _) in &files {
        let path = out_dir.join(name); // Build the full path for each list file we are about to refresh
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?; // Delete stale list files so rewrites never append to old content
        } // Close the exists branch for this output name
    } // Close the removal loop over declared output names

    for (name, lines) in &files {
        let path = out_dir.join(name); // Build the full path for the list file we are creating now
        let mut f = fs::File::create(&path).with_context(|| format!("create {}", path.display()))?; // Create or truncate the destination text file for writing
        for line in *lines {
            writeln!(f, "{line}").with_context(|| format!("write {}", path.display()))?; // Write one CIDR or host string per line using platform newline rules
        } // Close the per-line write loop
    } // Close the outer loop over each declared output file

    log::info!( // Emit an informational log entry summarizing how many prefixes were written
        "wrote {} IPv4 and {} IPv6 prefixes to {}", // Log template showing IPv4 count, IPv6 count, and folder path
        ipv4_cidr.len(), // Provide the IPv4 prefix count for the first placeholder in the template
        ipv6_cidr.len(), // Provide the IPv6 prefix count for the second placeholder in the template
        out_dir.display() // Provide the human-readable output directory for the third placeholder in the template
    ); // Close the log macro invocation

    Ok(()) // Return success without extra data now that every list file is refreshed
} // Close the run function

pub fn run_private_ips(out_dir: &Path) -> Result<()> {
    let url = std::env::var("ROUST_PRIVATE_IPS_JSON_URL").unwrap_or_else(|_| PRIVATE_IPS_JSON_URL.to_string()); // Allow operators to override the private IP JSON download URL via environment
    let body = http_get_string(&url).context("private_ips JSON")?; // Download the private IP JSON document as a UTF-8 string

    let json_path = out_dir.join("private_ips.json"); // Build the output path for the raw private IP JSON snapshot
    fs::write(&json_path, &body).with_context(|| format!("write {}", json_path.display()))?; // Persist the downloaded JSON bytes next to the generated text lists

    let cidrs: Vec<String> = serde_json::from_str(&body).context("parse private_ips.json (expected JSON array of strings)")?; // Parse the JSON array into owned CIDR strings

    let plain: Vec<String> = cidrs.iter().map(|s| strip_subnet(s)).collect(); // Convert each CIDR into a host-only string for the plain list output

    let files: [(&str, &[String]); 2] = [("private_ips_cidr.txt", &cidrs), ("private_ips.txt", &plain)]; // Declare the two private list files and their backing slices

    for (name, _) in &files {
        let path = out_dir.join(name); // Build the full path for each private list file we are refreshing
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?; // Remove any old private list file before rewriting it cleanly
        } // Close the exists branch for this private output name
    } // Close the removal loop for private list outputs

    for (name, lines) in &files {
        let path = out_dir.join(name); // Build the full path for the private list file we are writing now
        let mut f = fs::File::create(&path).with_context(|| format!("create {}", path.display()))?; // Create or truncate the private list destination file
        for line in *lines {
            writeln!(f, "{line}").with_context(|| format!("write {}", path.display()))?; // Write each private network entry on its own text line
        } // Close the per-line write loop for this private list file
    } // Close the outer loop over each private list file

    log::info!( // Emit an informational log entry summarizing how many private CIDR rows were written
        "wrote {} private IP CIDR rows under {}", // Log template with a count and destination folder path
        cidrs.len(), // Provide the number of private CIDR strings for the first placeholder
        out_dir.display() // Provide the output directory display string for the second placeholder
    ); // Close the log macro invocation

    Ok(()) // Return success after private list files are fully written
} // Close the run_private_ips function

fn strip_subnet(cidr: &str) -> String {
    cidr.split('/') // Split the CIDR token on the slash between host bits and prefix length
        .next() // Take only the host portion before the first slash if a slash exists
        .unwrap_or(cidr) // Fall back to the full token when no slash is present in the input
        .trim() // Remove stray leading or trailing whitespace characters from the host token
        .to_string() // Convert the trimmed string slice into an owned String for callers
} // Close the strip_subnet function

#[cfg(test)] // Compile the following tests only when the test harness is building this crate
mod tests {
    use super::*; // Bring the parent module items into scope for concise test code

    #[test] // Mark the following function as a unit test for IPv4 subnet stripping
    fn strip_subnet_ipv4() {
        assert_eq!(strip_subnet("2.57.3.0/24"), "2.57.3.0"); // Expect the IPv4 host portion before the slash for a typical mask
        assert_eq!(strip_subnet("2.144.0.0/14"), "2.144.0.0"); // Expect correct stripping for a shorter prefix length than a slash twenty-four
    } // Close strip_subnet_ipv4 test

    #[test] // Mark the following function as a unit test for IPv6 subnet stripping
    fn strip_subnet_ipv6() {
        assert_eq!(strip_subnet("2001:678:b0::/46"), "2001:678:b0::"); // Expect the IPv6 host portion without the prefix length suffix
    } // Close strip_subnet_ipv6 test

    #[test] // Mark the following function as a unit test for parsing a minimal aggregated payload
    fn parse_sample_payload() {
        let j = r#"{"country":"Iran","countryCode":"IR","prefixes":{"ipv4":["1.2.3.0/24"],"ipv6":["2001:db8::/32"]}}"#; // Hold compact JSON that mirrors the real remote schema for tests
        let p: AggregatedPayload = serde_json::from_str(j).unwrap(); // Parse the sample JSON and panic if deserialization fails inside tests
        assert_eq!(p.prefixes.ipv4, vec!["1.2.3.0/24"]); // Assert the IPv4 list matches the single CIDR we embedded in the sample JSON
        assert_eq!(p.prefixes.ipv6, vec!["2001:db8::/32"]); // Assert the IPv6 list matches the single CIDR we embedded in the sample JSON
    } // Close parse_sample_payload test

    #[test] // Mark the following function as a unit test for parsing a JSON array of CIDR strings
    fn parse_private_ips_payload() {
        let j = r#"["10.0.0.0/8","172.16.0.0/12"]"#; // Hold a two-element JSON array of RFC1918-style CIDR strings for parsing tests
        let v: Vec<String> = serde_json::from_str(j).unwrap(); // Parse the JSON array into owned strings and panic if parsing fails
        assert_eq!(v.len(), 2); // Assert the array length matches the number of literals we placed in the sample JSON text
    } // Close parse_private_ips_payload test
} // Close the tests module
