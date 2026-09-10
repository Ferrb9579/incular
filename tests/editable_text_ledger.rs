//! Validate the EditableText retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly, plus targeted negatives proving
//! fluent discovery neither misses options nor misclassifies observer
//! getters as options.
mod ledger;

use ledger::{DiscoveryStyle, FamilySpec, LedgerError, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const EDITABLE_TEXT_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "EditableText",
        file: "crates/incular-widgets/src/tree/descriptors.rs",
        style: DiscoveryStyle::MethodNames,
    }],
};

const FIXTURE_FILE: &str = "crates/incular-widgets/src/tree/descriptors.rs";

const FIXTURE_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "EditableText",
        file: FIXTURE_FILE,
        style: DiscoveryStyle::MethodNames,
    }],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/editable_text_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn editable_text_ledger_validates() {
    if let Err(errors) = validate_ledger(&real_ledger(), &repository_root(), &EDITABLE_TEXT_FAMILY)
    {
        panic!(
            "editable text ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Unique scratch root per test so parallel cases never share fixtures.
fn fixture_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "incular-ledger-fixture-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("crates/fixture")).unwrap();
    root
}

fn write_source(root: &Path, name: &str, text: &str) {
    std::fs::write(root.join("crates/fixture").join(name), text).unwrap();
}

/// Minimal well-formed ledger over fixture sources: a miniature
/// `descriptors.rs` with one struct, two options, and an observer getter
/// that must never count as an option.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "Fixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["layout"],
        "records": [
            {
                "symbol": "EditableText",
                "option": "controller",
                "default": "required, no default",
                "validation": "constructor parameter",
                "conversion": {
                    "path": "crates/fixture/convert.rs",
                    "kind": "trait_impl",
                    "trait": "From",
                    "target": "Widget",
                    "source": "EditableText"
                },
                "retained_owner": {
                    "path": "crates/fixture/owner.rs",
                    "kind": "struct",
                    "name": "FixturePolicy"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "layout": [
                        {
                            "path": "crates/fixture/owner.rs",
                            "kind": "method",
                            "owner": "WidgetTree",
                            "name": "layout_fixture"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_renders"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "EditableText",
                "option": "multiline",
                "default": false,
                "validation": "bool setter",
                "conversion": {
                    "path": "crates/fixture/convert.rs",
                    "kind": "trait_impl",
                    "trait": "From",
                    "target": "Widget",
                    "source": "EditableText"
                },
                "retained_owner": {
                    "path": "crates/fixture/owner.rs",
                    "kind": "struct",
                    "name": "FixturePolicy"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "layout": [
                        {
                            "path": "crates/fixture/owner.rs",
                            "kind": "method",
                            "owner": "WidgetTree",
                            "name": "layout_fixture"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_renders"}
                ],
                "status": "implemented"
            }
        ]
    })
}

fn write_descriptors(root: &Path, text: &str) {
    std::fs::create_dir_all(root.join("crates/incular-widgets/src/tree")).unwrap();
    std::fs::write(
        root.join("crates/incular-widgets/src/tree/descriptors.rs"),
        text,
    )
    .unwrap();
}

fn fixture_sources(root: &Path) {
    // Completeness discovers options from this exact path, so the fixture
    // carries a miniature descriptor whose public API matches the fixture
    // ledger exactly: `new(controller)` plus the `multiline` setter, with
    // an observer getter that must never count as an option.
    write_descriptors(
        root,
        "pub struct EditableText;\nimpl EditableText {\n    pub fn new(controller: Controller) -> Self {\n        let _ = controller;\n        unimplemented!()\n    }\n    pub fn multiline(mut self, multiline: bool) -> Self {\n        let _ = multiline;\n        unimplemented!()\n    }\n    pub fn is_multiline(&self) -> bool {\n        unimplemented!()\n    }\n}\npub struct Controller;\n",
    );
    write_source(
        root,
        "convert.rs",
        "pub struct EditableText;\npub struct Widget;\nimpl From<EditableText> for Widget {\n    fn from(_: EditableText) -> Self { Widget }\n}\n",
    );
    write_source(
        root,
        "convert.rs",
        "pub struct EditableText;\npub struct Widget;\nimpl From<EditableText> for Widget {\n    fn from(_: EditableText) -> Self { Widget }\n}\n",
    );
    write_source(
        root,
        "owner.rs",
        "pub struct FixturePolicy;\npub struct WidgetTree;\nimpl WidgetTree {\n    pub fn layout_fixture(&self) {}\n}\n",
    );
    write_source(
        root,
        "tests.rs",
        "#[test]\nfn fixture_renders() {}\nfn fixture_helper() {}\n",
    );
}

fn errors_for(ledger: &Value, root: &Path) -> Vec<LedgerError> {
    validate_ledger(ledger, root, &FIXTURE_FAMILY).expect_err("fixture must be rejected")
}

#[test]
fn validator_accepts_a_well_formed_fixture() {
    let root = fixture_root("positive");
    fixture_sources(&root);
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root, &FIXTURE_FAMILY) {
        panic!("well-formed fixture must validate: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_missing_and_stale_options() {
    // A ledger without the `multiline` record misses discovered public API.
    // The observer getter `is_multiline` must not count as an option.
    let root = fixture_root("missing");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["option"] == "multiline")
        .unwrap();
    records.remove(position);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "EditableText".to_owned(),
            option: "multiline".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);

    // A record for a removed option is stale.
    let root = fixture_root("stale");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["records"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "symbol": "EditableText",
            "option": "removed_option",
            "default": false,
            "validation": "gone",
            "conversion": {
                "path": "crates/fixture/convert.rs",
                "kind": "trait_impl",
                "trait": "From",
                "target": "Widget",
                "source": "EditableText"
            },
            "retained_owner": {
                "path": "crates/fixture/owner.rs",
                "kind": "struct",
                "name": "FixturePolicy"
            },
            "comparison": {"prose": "none", "refs": []},
            "consumers": {
                "layout": [
                    {
                        "path": "crates/fixture/owner.rs",
                        "kind": "method",
                        "owner": "WidgetTree",
                        "name": "layout_fixture"
                    }
                ]
            },
            "regressions": [
                {"path": "crates/fixture/tests.rs", "name": "fixture_renders"}
            ],
            "status": "implemented"
        }));
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::StaleOption {
            symbol: "EditableText".to_owned(),
            option: "removed_option".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_unresolved_references() {
    // A consumer naming a missing method fails even though every option
    // is recorded.
    let root = fixture_root("unresolved");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["records"][0]["consumers"]["layout"][0]["name"] =
        Value::String("no_such_consumer".to_owned());
    let errors = errors_for(&ledger, &root);
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, LedgerError::UnresolvedReference { .. })),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
