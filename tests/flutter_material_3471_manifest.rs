//! Contract checks for the pinned Flutter 3.47.1 Material inventory.

use std::{collections::HashSet, fs, path::Path};

const ALLOWED: [&str; 8] = [
    "EXACT",
    "RUSTIFIED",
    "MERGED_CORE",
    "MERGED_CONTROLS",
    "DEFERRED",
    "SKIPPED_LEGACY",
    "SKIPPED_PLATFORM",
    "INTERNAL",
];

fn json_string(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('\"')?;
    Some(rest[..end].to_owned())
}

#[test]
fn pinned_material_graph_is_complete_and_explicit() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/flutter_material_3471_parity.jsonl");
    let source = fs::read_to_string(path).expect("Flutter Material parity graph must exist");
    let mut ids = HashSet::new();
    let mut rows = 0_usize;
    let mut rustified = 0_usize;

    for (line_number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        rows += 1;
        let id =
            json_string(line, "id").unwrap_or_else(|| panic!("line {} has no id", line_number + 1));
        assert!(
            ids.insert(id),
            "duplicate Material graph row at line {}",
            line_number + 1
        );
        assert_eq!(
            json_string(line, "flutter_version").as_deref(),
            Some("3.47.1")
        );
        assert!(json_string(line, "source").is_some());
        assert!(json_string(line, "behavior_evidence").is_some());
        assert!(json_string(line, "baseline_evidence").is_some());
        assert!(json_string(line, "diff_evidence").is_some());
        let status = json_string(line, "status").expect("status");
        assert!(
            ALLOWED.contains(&status.as_str()),
            "unknown Material status {status}"
        );
        if status == "RUSTIFIED" {
            rustified += 1;
            assert!(json_string(line, "incular").is_some());
            assert!(json_string(line, "compile_evidence").is_some());
            assert!(json_string(line, "test_evidence").is_some());
        }
        assert!(!line.contains("UNKNOWN") && !line.contains("UNRESOLVED"));
        assert!(!line.contains("TODO") && !line.contains("todo"));
    }

    assert!(
        rows >= 500,
        "Material inventory unexpectedly shortened: {rows}"
    );
    assert!(
        rustified >= 50,
        "implemented Material foundation is missing from the graph"
    );
}
