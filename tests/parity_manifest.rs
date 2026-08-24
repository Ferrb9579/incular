//! Contract test for the checked-in Flutter-derived parity inventory.
//!
//! The manifest uses one compact JSON object per line. It deliberately avoids
//! a parser dependency in the workspace test crate while remaining consumable
//! by normal JSON-lines tooling.

use std::{collections::HashSet, fs, path::Path};

const REQUIRED_FIELDS: [&str; 7] = [
    "flutter",
    "equivalent",
    "status",
    "owner",
    "public_path",
    "evidence",
    "notes",
];
const STATES: [&str; 5] = ["IMPLEMENTED", "MERGED", "INTERNAL", "DEFERRED", "SKIPPED"];

#[test]
fn widget_parity_manifest_is_complete_and_has_only_deliberate_gaps() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/widget_parity.jsonl");
    let source = fs::read_to_string(&path).expect("widget parity manifest must be checked in");
    let mut capabilities = HashSet::new();
    let mut totals = [0_usize; 5];

    for (line_number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.starts_with('{') && line.ends_with('}'),
            "manifest line {} is not a JSON object",
            line_number + 1
        );
        let mut values = Vec::new();
        for field in REQUIRED_FIELDS {
            let value = json_string_field(line, field).unwrap_or_else(|| {
                panic!("manifest line {} is missing {field:?}", line_number + 1)
            });
            assert!(
                !value.trim().is_empty(),
                "manifest line {} has an empty {field:?}",
                line_number + 1
            );
            assert!(
                !value.contains("TODO") && !value.contains("todo"),
                "manifest line {} uses an unresolved TODO in {field:?}",
                line_number + 1
            );
            values.push(value);
        }
        let capability = values[0];
        assert!(
            capabilities.insert(capability),
            "duplicate Flutter capability {capability:?} in manifest"
        );
        let status = values[2];
        let index = STATES
            .iter()
            .position(|known| *known == status)
            .unwrap_or_else(|| {
                panic!(
                    "manifest line {} has unresolved or unknown status {status:?}",
                    line_number + 1
                )
            });
        totals[index] += 1;
        if matches!(status, "DEFERRED" | "SKIPPED") {
            assert!(
                values[6].len() > 12,
                "manifest line {} needs a specific rationale for {status}",
                line_number + 1
            );
        }
        if status == "DEFERRED" {
            assert!(
                values[6].to_ascii_lowercase().contains("prerequisite"),
                "manifest line {} must name the prerequisite for DEFERRED work",
                line_number + 1
            );
        }
        if matches!(status, "IMPLEMENTED" | "MERGED") {
            assert!(
                values[5].contains("test") || values[5].contains("examples/"),
                "manifest line {} needs behavioral evidence for {status}",
                line_number + 1
            );
        }
    }

    assert!(
        capabilities.len() >= 90,
        "inventory was unexpectedly shortened"
    );
    eprintln!(
        "widget parity: implemented: {}, merged: {}, internal: {}, deferred: {}, skipped: {}, unresolved: 0",
        totals[0], totals[1], totals[2], totals[3], totals[4]
    );
}

const FLUTTER_API_REQUIRED_FIELDS: [&str; 12] = [
    "flutter",
    "incular",
    "category",
    "status",
    "constructor_parity",
    "parameter_parity",
    "default_parity",
    "behavior_parity",
    "flutter_docs",
    "public_path",
    "evidence",
    "notes",
];

#[test]
fn flutter_api_parity_manifest_is_complete_and_has_only_deliberate_gaps() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/flutter_api_parity.jsonl");
    let source = fs::read_to_string(&path).expect("flutter API parity manifest must be checked in");
    let mut capabilities = HashSet::new();
    let mut totals = [0_usize; 5];

    for (line_number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.starts_with('{') && line.ends_with('}'),
            "manifest line {} is not a JSON object",
            line_number + 1
        );
        let mut values = Vec::new();
        for field in FLUTTER_API_REQUIRED_FIELDS {
            let value = json_string_field(line, field).unwrap_or_else(|| {
                panic!("manifest line {} is missing {field:?}", line_number + 1)
            });
            assert!(
                !value.trim().is_empty(),
                "manifest line {} has an empty {field:?}",
                line_number + 1
            );
            assert!(
                !value.contains("TODO") && !value.contains("todo"),
                "manifest line {} uses an unresolved TODO in {field:?}",
                line_number + 1
            );
            values.push(value);
        }
        let capability = values[0];
        assert!(
            capabilities.insert(capability),
            "duplicate Flutter API {capability:?} in manifest"
        );
        let status = values[3];
        let index = STATES
            .iter()
            .position(|known| *known == status)
            .unwrap_or_else(|| {
                panic!(
                    "manifest line {} has unresolved or unknown status {status:?}",
                    line_number + 1
                )
            });
        totals[index] += 1;
        if matches!(status, "DEFERRED" | "SKIPPED") {
            assert!(
                values[11].len() > 12,
                "manifest line {} needs a specific rationale for {status}",
                line_number + 1
            );
        }
    }

    assert!(
        capabilities.len() >= 80,
        "Flutter API parity manifest was unexpectedly shortened"
    );
    eprintln!(
        "flutter API parity: implemented: {}, merged: {}, internal: {}, deferred: {}, skipped: {}, unresolved: 0",
        totals[0], totals[1], totals[2], totals[3], totals[4]
    );
}

fn json_string_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("\"{field}\":\"");
    let start = line.find(&prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}
