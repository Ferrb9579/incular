//! Contract checks for the source-derived Flutter 3.47.1 Widgets export graph.

use serde_json::Value;
use std::{collections::HashSet, fs, path::Path};

const STATUSES: [&str; 8] = [
    "EXACT",
    "RUSTIFIED",
    "MERGED",
    "INTERNAL",
    "DEFERRED_PLATFORM",
    "DEFERRED_RENDERER",
    "SKIPPED_DART_MECHANIC",
    "SKIPPED_DEPRECATED",
];
const MEMBER_DEPTHS: [&str; 6] = [
    "FULL_MEMBER_AUDIT",
    "INHERITANCE_MECHANIC_MAPPING",
    "NO_MEANINGFUL_APPLICATION_MEMBERS",
    "MERGED_INTO",
    "DEFERRED_PLATFORM",
    "SKIPPED",
];
const PORT_POLICIES: [&str; 11] = [
    "DIRECT",
    "RUSTIFIED",
    "MERGED_SIGNAL",
    "MERGED_CONTEXT",
    "MERGED_TOKIO",
    "MERGED_DOMAIN",
    "INTERNAL",
    "SKIP_DART_MECHANIC",
    "SKIP_LOW_VALUE",
    "DEFER_PLATFORM",
    "DEFER_RENDERER",
];
const PRIORITIES: [&str; 5] = [
    "P0_DEPLOYABLE",
    "P1_COMMON",
    "P2_ADVANCED",
    "P3_COMPATIBILITY",
    "NEVER_PORT",
];

fn graph_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("specs")
        .join(name)
}

#[test]
fn widgets_graph_is_pinned_to_flutter_source() {
    let exports: Value = serde_json::from_str(
        &fs::read_to_string(graph_path("flutter_widgets_3471_exports.json"))
            .expect("Widgets export graph must exist"),
    )
    .expect("Widgets export graph must be JSON");
    assert_eq!(exports["flutter_tag"], "3.47.1");
    assert_eq!(
        exports["flutter_commit"],
        "6655482ec06e547f90abf8ae7590466f4415978d"
    );
    assert_eq!(exports["source"], "packages/flutter/lib/widgets.dart");
    assert_eq!(exports["direct_export_count"], 173);
    assert_eq!(exports["recursive_symbol_count"], 1_307);
    assert_eq!(exports["exports"].as_array().map(Vec::len), Some(173));
    assert!(exports["symbol_kind_counts"].is_object());
    assert!(exports["application_facing_count"].as_u64().is_some());
    assert!(exports["platform_specific_count"].as_u64().is_some());
    assert!(exports["deprecated_count"].as_u64().is_some());
}

#[test]
fn every_recursive_row_has_an_explicit_status_and_owner() {
    let source = fs::read_to_string(graph_path("flutter_widgets_3471_parity.jsonl"))
        .expect("Widgets parity graph must exist");
    let mut symbols = HashSet::new();
    let mut rows = 0_usize;
    for (line_number, line) in source.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("invalid Widgets row {}: {error}", line_number + 1));
        rows += 1;
        let symbol = row["flutter_symbol"].as_str().expect("symbol").to_owned();
        assert!(
            symbols.insert(symbol.clone()),
            "duplicate Widgets symbol {symbol}"
        );
        let status = row["status"].as_str().expect("status");
        assert!(
            STATUSES.contains(&status),
            "unknown Widgets status {status}"
        );
        assert!(row["flutter_source"].as_str().is_some());
        assert!(row["incular_owner_crate"].as_str().is_some());
        assert!(row["deprecated"].as_bool().is_some());
        assert!(row["constructor_evidence"].as_str().is_some());
        assert!(row["member_evidence"].as_str().is_some());
        assert!(row["default_evidence"].as_str().is_some());
        assert!(row["behavior_evidence"].as_str().is_some());
        assert!(row["test_evidence"].as_str().is_some());
        let port_policy = row["port_policy"].as_str().expect("port policy");
        assert!(
            PORT_POLICIES.contains(&port_policy),
            "unknown Widgets port policy {port_policy}"
        );
        assert!(row["port_policy_evidence"].as_str().is_some());
        let priority = row["priority"].as_str().expect("priority");
        assert!(
            PRIORITIES.contains(&priority),
            "unknown Widgets priority {priority}"
        );
        assert!(row["priority_evidence"].as_str().is_some());
        let member_depth = row["member_depth"].as_str().expect("member depth");
        assert!(
            MEMBER_DEPTHS.contains(&member_depth),
            "unknown member depth {member_depth}"
        );
        assert!(!line.contains("TODO"));
        assert!(!line.contains("UNKNOWN"));
        assert!(!line.contains("UNRESOLVED"));
        assert!(!line.contains("MISSING"));
        if status == "MERGED" && row["incular_owner_crate"] != "characters" {
            assert!(
                row["incular_public_path"].as_str().is_some(),
                "merged row {symbol} must expose its authoritative public mapping"
            );
            if row["member_depth"] == "FULL_MEMBER_AUDIT" {
                assert!(
                    row["member_evidence"]
                        .as_str()
                        .is_some_and(|evidence| evidence.contains("flutter_member_parity.jsonl")),
                    "full member audit for {symbol} must cite the member manifest"
                );
            }
        }
    }
    assert_eq!(
        rows, 1_307,
        "recursive export graph changed; regenerate from the pinned source"
    );
}

#[test]
fn task23_policy_and_priority_manifest_is_complete() {
    let source = fs::read_to_string(graph_path("flutter_widgets_3471_parity.jsonl")).unwrap();
    let rows: Vec<Value> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let policies = rows
        .iter()
        .filter_map(|row| row["port_policy"].as_str())
        .collect::<HashSet<_>>();
    let priorities = rows
        .iter()
        .filter_map(|row| row["priority"].as_str())
        .collect::<HashSet<_>>();
    assert_eq!(policies.len(), PORT_POLICIES.len());
    assert!(PORT_POLICIES.iter().all(|policy| policies.contains(policy)));
    assert_eq!(priorities.len(), PRIORITIES.len());
    assert!(
        PRIORITIES
            .iter()
            .all(|priority| priorities.contains(priority))
    );

    // These decisions are the permanent Task 23 architecture contract.
    for (symbol, policy, priority) in [
        ("ChangeNotifier", "MERGED_SIGNAL", "NEVER_PORT"),
        ("ValueNotifier", "MERGED_SIGNAL", "NEVER_PORT"),
        ("ValueListenableBuilder", "MERGED_SIGNAL", "NEVER_PORT"),
        ("ListenableBuilder", "MERGED_SIGNAL", "NEVER_PORT"),
        ("StatefulWidget", "SKIP_DART_MECHANIC", "NEVER_PORT"),
        ("StatelessWidget", "SKIP_DART_MECHANIC", "NEVER_PORT"),
        ("GlobalKey", "SKIP_DART_MECHANIC", "NEVER_PORT"),
        ("InheritedWidget", "MERGED_CONTEXT", "NEVER_PORT"),
        ("InheritedModel", "MERGED_CONTEXT", "NEVER_PORT"),
        ("TickerProviderStateMixin", "INTERNAL", "NEVER_PORT"),
        ("AutomaticKeepAliveClientMixin", "INTERNAL", "NEVER_PORT"),
        ("RestorationMixin", "MERGED_DOMAIN", "NEVER_PORT"),
        ("FutureBuilder", "MERGED_TOKIO", "NEVER_PORT"),
        ("StreamBuilder", "MERGED_TOKIO", "NEVER_PORT"),
        ("Container", "DIRECT", "P0_DEPLOYABLE"),
        ("Text", "DIRECT", "P0_DEPLOYABLE"),
        ("ScrollPhysics", "MERGED_DOMAIN", "P0_DEPLOYABLE"),
        ("CustomPainter", "RUSTIFIED", "P0_DEPLOYABLE"),
        ("IntrinsicWidth", "RUSTIFIED", "P1_COMMON"),
        ("SelectableRegion", "RUSTIFIED", "P1_COMMON"),
        ("TreeSliver", "RUSTIFIED", "P2_ADVANCED"),
        ("SliverCrossAxisGroup", "RUSTIFIED", "P2_ADVANCED"),
        ("TwoDimensionalViewport", "RUSTIFIED", "P2_ADVANCED"),
    ] {
        let row = rows
            .iter()
            .find(|row| row["flutter_symbol"] == symbol)
            .unwrap_or_else(|| panic!("missing Task 23 symbol {symbol}"));
        assert_eq!(row["port_policy"], policy, "wrong policy for {symbol}");
        assert_eq!(row["priority"], priority, "wrong priority for {symbol}");
    }

    let summary: Value = serde_json::from_str(
        &fs::read_to_string(graph_path("P0_DEPLOYABLE_WIDGETS_SUMMARY.json"))
            .expect("P0 summary metadata must exist"),
    )
    .expect("P0 summary metadata must be JSON");
    assert_eq!(summary["canonical_row_count"], 1_307);
    assert_eq!(summary["flutter_tag"], "3.47.1");
    assert_eq!(
        summary["flutter_commit"],
        "6655482ec06e547f90abf8ae7590466f4415978d"
    );
    let p0_source = fs::read_to_string(graph_path("P0_DEPLOYABLE_WIDGETS.jsonl"))
        .expect("P0 deployable queue must exist");
    let p0_rows: Vec<Value> = p0_source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        p0_rows.len() as u64,
        summary["p0_deployable_count"].as_u64().unwrap()
    );
    assert!(p0_rows.iter().all(|row| row["priority"] == "P0_DEPLOYABLE"));
    assert!(p0_rows.iter().all(|row| row["priority"] != "NEVER_PORT"));
    let canonical_symbols = rows
        .iter()
        .map(|row| row["flutter_symbol"].as_str().unwrap())
        .collect::<HashSet<_>>();
    let mut p0_symbols = HashSet::new();
    for row in p0_rows {
        let symbol = row["flutter_symbol"].as_str().unwrap().to_owned();
        assert!(canonical_symbols.contains(symbol.as_str()));
        assert!(
            p0_symbols.insert(symbol.clone()),
            "duplicate P0 symbol {symbol}"
        );
    }
}

#[test]
fn representative_domain_ownership_is_not_duplicated_in_widgets() {
    let source = fs::read_to_string(graph_path("flutter_widgets_3471_parity.jsonl")).unwrap();
    let rows: Vec<Value> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for (symbol, owner) in [
        ("UniqueKey", "incular-core"),
        ("Alignment", "incular-config"),
        ("AnimationController", "incular-animation"),
        ("TextStyle", "incular-text"),
        ("ScrollPhysics", "incular-scroll"),
    ] {
        let row = rows
            .iter()
            .find(|row| row["flutter_symbol"] == symbol)
            .unwrap_or_else(|| panic!("missing representative row {symbol}"));
        assert_eq!(
            row["incular_owner_crate"], owner,
            "wrong owner for {symbol}"
        );
    }
}

#[test]
fn graph_keeps_sdk_reexports_and_deprecations_explicit() {
    let source = fs::read_to_string(graph_path("flutter_widgets_3471_parity.jsonl")).unwrap();
    let rows: Vec<Value> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for (symbol, owner) in [("Color", "incular-core"), ("Canvas", "incular-rendering")] {
        let row = rows
            .iter()
            .find(|row| row["flutter_symbol"] == symbol)
            .unwrap_or_else(|| panic!("missing dart:ui re-export {symbol}"));
        assert_eq!(row["incular_owner_crate"], owner);
        assert_eq!(row["flutter_source"], "packages/flutter/lib/dart:ui");
    }
    let will_pop = rows
        .iter()
        .find(|row| row["flutter_symbol"] == "WillPopScope")
        .expect("deprecated Widgets symbol");
    assert_eq!(will_pop["deprecated"], true);
    assert_eq!(will_pop["status"], "SKIPPED_DEPRECATED");
}

#[test]
fn graph_contains_widgets_entering_through_recursive_exports() {
    let source = fs::read_to_string(graph_path("flutter_widgets_3471_parity.jsonl")).unwrap();
    for symbol in [
        "Widget",
        "Container",
        "Row",
        "Text",
        "EditableText",
        "RawRadio",
        "RawScrollbar",
        "RawTooltip",
        "AnimationController",
        "ScrollPhysics",
        "TextStyle",
        "BoxDecoration",
    ] {
        assert!(
            source
                .lines()
                .any(|line| line.contains(&format!("\"flutter_symbol\": \"{symbol}\""))),
            "recursive graph is missing {symbol}"
        );
    }
}

#[test]
fn task24_closes_exact_p0_deferred_intersection() {
    const TASK24: [&str; 35] = [
        "Action",
        "Actions",
        "AlwaysScrollableScrollPhysics",
        "BouncingScrollPhysics",
        "FilterQuality",
        "FocusScopeNode",
        "FontFeature",
        "FontVariation",
        "FontWeight",
        "FormState",
        "IconData",
        "ImageConfiguration",
        "Intent",
        "KeyboardListener",
        "Localizations",
        "MediaQueryData",
        "ModalRoute",
        "NeverScrollableScrollPhysics",
        "OverlayRoute",
        "PageRoute",
        "PageRouteBuilder",
        "PageScrollPhysics",
        "Paint",
        "PinnedHeaderSliver",
        "PopupRoute",
        "RangeMaintainingScrollPhysics",
        "RawDialogRoute",
        "RestorationScope",
        "RotationTransition",
        "RouteSettings",
        "Shader",
        "Shadow",
        "Shortcuts",
        "TransitionRoute",
        "UndoHistoryController",
    ];
    let source = fs::read_to_string(graph_path("P0_DEPLOYABLE_WIDGETS.jsonl")).unwrap();
    let rows: Vec<Value> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let expected: HashSet<_> = TASK24.into_iter().collect();
    let closure_rows: Vec<_> = rows
        .iter()
        .filter(|row| {
            row["p0_closure_status"]
                .as_str()
                .is_some_and(|value| value != "NOT_IN_SCOPE")
        })
        .collect();
    assert_eq!(closure_rows.len(), expected.len());
    assert_eq!(
        closure_rows
            .iter()
            .map(|row| row["flutter_symbol"].as_str().unwrap())
            .collect::<HashSet<_>>(),
        expected
    );
    assert!(closure_rows.iter().all(|row| {
        row["p0_closure_status"] != "TRUE_RENDERER_DEFERRED"
            && row["status"] != "DEFERRED_RENDERER"
            && row["incular_public_path"].as_str().is_some()
    }));
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(graph_path("P0_DEPLOYABLE_WIDGETS_SUMMARY.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(summary["p0_deployable_count"], 207);
    assert_eq!(summary["p0_closure_expected_count"], 35);
    assert_eq!(summary["p0_closure_complete_count"], 35);
    assert_eq!(summary["p0_true_deferred_count"], 0);
    assert!(
        summary["p0_remaining_deferred_symbols"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
