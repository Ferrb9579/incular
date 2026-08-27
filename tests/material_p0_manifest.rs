//! Machine-readable contracts for the Material 3.47.1 P0 projection.

use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn rows(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("valid JSONL row"))
        .collect()
}

#[test]
fn material_canonical_graph_has_generated_policy_fields() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let canonical = rows(&root.join("specs/flutter_material_3471_parity.jsonl"));
    let priorities = [
        "P0_MATERIAL_DEPLOYABLE",
        "P1_MATERIAL_COMMON",
        "P2_MATERIAL_ADVANCED",
        "P3_MATERIAL_COMPATIBILITY",
    ];
    let policies = [
        "DIRECT",
        "RUSTIFIED",
        "MERGED_CONTROLS",
        "MERGED_CORE",
        "SKIP_LEGACY",
        "DEFER_PLATFORM",
    ];
    let mut ids = HashSet::new();
    for row in &canonical {
        let object = row.as_object().expect("object row");
        let id = object.get("id").and_then(Value::as_str).expect("row id");
        assert!(ids.insert(id), "duplicate canonical id: {id}");
        assert_eq!(
            object.get("flutter_version").and_then(Value::as_str),
            Some("3.47.1")
        );
        assert!(
            priorities.contains(
                &object
                    .get("priority")
                    .and_then(Value::as_str)
                    .expect("generated priority")
            )
        );
        assert!(
            policies.contains(
                &object
                    .get("port_policy")
                    .and_then(Value::as_str)
                    .expect("generated port policy")
            )
        );
        assert!(
            object
                .get("implementation_status")
                .and_then(Value::as_str)
                .is_some_and(|status| !status.is_empty())
        );
    }
    assert_eq!(canonical.len(), 551);
}

#[test]
fn material_p0_projection_and_summary_are_consistent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let canonical = rows(&root.join("specs/flutter_material_3471_parity.jsonl"));
    let p0 = rows(&root.join("specs/P0_MATERIAL_3471.jsonl"));
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(root.join("specs/P0_MATERIAL_3471_SUMMARY.json")).expect("summary"),
    )
    .expect("valid summary JSON");
    let canonical_ids: HashSet<_> = canonical
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str))
        .collect();
    let mut p0_ids = HashSet::new();
    for row in &p0 {
        let id = row.get("id").and_then(Value::as_str).expect("P0 id");
        assert!(
            canonical_ids.contains(id),
            "P0 id not in canonical graph: {id}"
        );
        assert!(p0_ids.insert(id), "duplicate P0 id: {id}");
        assert_eq!(
            row.get("priority").and_then(Value::as_str),
            Some("P0_MATERIAL_DEPLOYABLE")
        );
        assert_ne!(
            row.get("implementation_status").and_then(Value::as_str),
            Some("PARTIAL"),
            "generated P0 rows must use an explicit implementation/deferred status"
        );
    }
    assert_eq!(
        summary.get("canonical_row_count").and_then(Value::as_u64),
        Some(canonical.len() as u64)
    );
    assert_eq!(
        summary.get("p0_row_count").and_then(Value::as_u64),
        Some(p0.len() as u64)
    );
    assert_eq!(
        p0.len() as u64,
        summary
            .get("p0_row_count")
            .and_then(Value::as_u64)
            .expect("summary P0 row count")
    );
    assert!(p0.len() < canonical.len());

    // These are the app-facing P0 anchors. Supporting platform/renderer rows
    // may be explicitly DEFERRED_PLATFORM, but an ordinary application
    // component must point at the retained Material implementation rather
    // than silently reporting `incular: none`.
    let required = [
        "MaterialApp",
        "Scaffold",
        "AppBar",
        "Dialog",
        "SimpleDialog",
        "DropdownButton",
        "DropdownMenu",
        "MenuAnchor",
        "PopupMenuButton",
        "RangeSlider",
        "TabBarView",
        "InputDecorator",
        "OutlineInputBorder",
        "ProgressIndicatorThemeData",
        "SnackBar",
        "Tooltip",
        "Icons",
    ];
    for name in required {
        let row = p0
            .iter()
            .find(|row| row.get("flutter").and_then(Value::as_str) == Some(name))
            .unwrap_or_else(|| panic!("missing generated P0 anchor: {name}"));
        assert_ne!(
            row.get("implementation_status").and_then(Value::as_str),
            Some("DEFERRED_PLATFORM"),
            "app-facing P0 anchor remains platform-deferred: {name}"
        );
        assert!(
            row.get("incular")
                .and_then(Value::as_str)
                .is_some_and(|value| value.starts_with("incular_")),
            "app-facing P0 anchor has no Incular owner: {name}"
        );
    }

    // These branches are explicitly outside the frozen deployable surface;
    // keeping them in the canonical graph is useful for later work, but they
    // must not inflate the P0 acceptance projection.
    for excluded in [
        "BottomSheet",
        "DatePickerDialog",
        "showDatePicker",
        "showTimePicker",
        "SegmentedButton",
    ] {
        let row = canonical
            .iter()
            .find(|row| row.get("flutter").and_then(Value::as_str) == Some(excluded))
            .unwrap_or_else(|| panic!("missing canonical row: {excluded}"));
        assert_eq!(
            row.get("priority").and_then(Value::as_str),
            Some("P1_MATERIAL_COMMON"),
            "explicitly out-of-scope branch accidentally promoted: {excluded}"
        );
        assert!(!p0.iter().any(|candidate| {
            candidate.get("flutter").and_then(Value::as_str) == Some(excluded)
        }));
    }
}
