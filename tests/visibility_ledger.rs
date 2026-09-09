//! Validate the Visibility retained-property ledger against the tree.
//!
//! The ledger (`specs/visibility_properties.json`) records every exported
//! Visibility/Offstage option with its conversion, retained owner,
//! consumers, and named regressions. These checks keep the ledger honest:
//! reference targets must exist under repository-relative paths with
//! stable symbol names (never line numbers), and every claimed
//! regression must be a real test function. The ledger covers only the
//! Visibility family; passing here never claims whole-codebase
//! completeness.
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

fn ledger() -> Value {
    serde_json::from_str(include_str!("../specs/visibility_properties.json")).unwrap()
}

/// Split a `path/to/file.rs::Symbol` reference into its parts.
fn split_reference(reference: &str) -> (String, String) {
    let (path, symbol) = reference
        .split_once(".rs::")
        .unwrap_or_else(|| panic!("reference without .rs:: separator: {reference}"));
    (format!("{path}.rs"), symbol.to_owned())
}

/// Leading identifier of a symbol reference (`From<Visibility> for Widget`
/// checks `From`; `WidgetTree::update` checks `WidgetTree`).
fn symbol_head(symbol: &str) -> &str {
    let end = symbol
        .find(|character: char| !(character.is_alphanumeric() || character == '_'))
        .unwrap_or(symbol.len());
    &symbol[..end]
}

fn assert_reference(root: &Path, reference: &str) {
    let (path, symbol) = split_reference(reference);
    let text = fs::read_to_string(root.join(&path))
        .unwrap_or_else(|_| panic!("ledger references missing file {path}"));
    assert!(
        text.contains(symbol_head(&symbol)),
        "ledger references missing symbol {symbol} in {path}"
    );
}

/// Every `.rs::Symbol` token inside free-prose fields must resolve.
fn assert_prose_references(root: &Path, prose: &str) {
    for token in
        prose.split(|character: char| character.is_whitespace() || "(),;".contains(character))
    {
        if token.contains(".rs::") {
            assert_reference(root, token);
        }
    }
}

#[test]
fn visibility_ledger_schema_has_no_duplicates_and_known_dispositions() {
    let ledger = ledger();
    assert_eq!(ledger["schema_version"], 1);
    assert_eq!(ledger["family"], "Visibility");
    let mut seen = BTreeSet::new();
    for record in ledger["records"].as_array().unwrap() {
        let symbol = record["symbol"].as_str().unwrap();
        let option = record["option"].as_str().unwrap();
        assert!(
            seen.insert((symbol.to_owned(), option.to_owned())),
            "duplicate ledger entry {symbol}::{option}"
        );
        let status = record["status"].as_str().unwrap();
        assert!(
            ["implemented", "intentionally_unsupported", "unresolved"].contains(&status),
            "unknown disposition {status} for {symbol}::{option}"
        );
        assert!(
            record["default"].is_boolean()
                || record["default"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
            "empty default for {symbol}::{option}"
        );
        for field in ["validation", "conversion", "retained_owner", "comparison"] {
            assert!(
                record[field]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "empty {field} for {symbol}::{option}"
            );
        }
        if status == "implemented" {
            assert!(
                record["regressions"]
                    .as_array()
                    .is_some_and(|list| !list.is_empty()),
                "implemented {symbol}::{option} names no regression"
            );
            assert!(
                record["consumers"]
                    .as_object()
                    .is_some_and(|map| !map.is_empty()),
                "implemented {symbol}::{option} names no consumer"
            );
        }
    }
}

#[test]
fn visibility_ledger_references_resolve_to_stable_symbols() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ledger = ledger();
    for record in ledger["records"].as_array().unwrap() {
        let symbol = record["symbol"].as_str().unwrap();
        let option = record["option"].as_str().unwrap();
        let context = format!("{symbol}::{option}");
        assert_reference(root, record["conversion"].as_str().unwrap());
        assert_reference(root, record["retained_owner"].as_str().unwrap());
        assert_prose_references(root, record["comparison"].as_str().unwrap());
        for (phase, target) in record["consumers"].as_object().unwrap() {
            assert_reference(root, target.as_str().unwrap());
            let _ = (context.clone(), phase);
        }
        for regression in record["regressions"].as_array().unwrap() {
            let regression = regression.as_str().unwrap();
            let (path, test) = split_reference(regression);
            let text = fs::read_to_string(root.join(&path))
                .unwrap_or_else(|_| panic!("regression in missing file {path}"));
            assert!(
                text.contains(&format!("fn {test}")),
                "regression {test} is not a test function in {path}"
            );
        }
    }
}
