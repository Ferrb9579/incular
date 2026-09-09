//! Validate the Visibility retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly, and fixtures prove each
//! rejection fires.
mod ledger;

use ledger::{DiscoveryStyle, FamilySpec, LedgerError, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const VISIBILITY_FILE: &str = "crates/incular-widgets/src/layout/basic/visibility.rs";

const VISIBILITY_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Visibility",
            file: VISIBILITY_FILE,
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Offstage",
            file: VISIBILITY_FILE,
            style: DiscoveryStyle::MethodNames,
        },
    ],
};

const FIXTURE_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "Visibility",
        file: VISIBILITY_FILE,
        style: DiscoveryStyle::MethodNames,
    }],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/visibility_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn visibility_ledger_validates() {
    if let Err(errors) = validate_ledger(&real_ledger(), &repository_root(), &VISIBILITY_FAMILY) {
        panic!(
            "visibility ledger invalid:\n{}",
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
/// `visibility.rs` with one struct, two options, and a generated builder,
/// plus one consumer and two regressions. Negative cases mutate a copy.
/// `syn` parses syntax only, so undefined types in fixtures are fine.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "Fixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["layout"],
        "builder_parity": {
            "Visibility": {
                "path": "crates/fixture/tests.rs",
                "name": "fixture_builder_parity"
            }
        },
        "records": [
            {
                "symbol": "Visibility",
                "option": "visible",
                "default": true,
                "validation": "bool setter",
                "conversion": {
                    "path": "crates/fixture/convert.rs",
                    "kind": "trait_impl",
                    "trait": "From",
                    "target": "Widget",
                    "source": "Visibility"
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
                    {"path": "crates/fixture/tests.rs", "name": "fixture_renders"},
                    {"path": "crates/fixture/tests.rs", "name": "fixture_builder_parity"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Visibility",
                "option": "child",
                "default": "required, no default",
                "validation": "required conversion parameter",
                "conversion": {
                    "path": "crates/fixture/convert.rs",
                    "kind": "trait_impl",
                    "trait": "From",
                    "target": "Widget",
                    "source": "Visibility"
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

fn fixture_sources(root: &Path) {
    // Completeness discovers options from this exact path, so fixtures
    // carry a miniature one whose public API matches the fixture ledger
    // exactly: `new(child)` plus the `visible` setter, with a generated
    // builder covered by the explicit builder_parity reference.
    write_visibility(root, MINIATURE_VISIBILITY);
    write_source(
        root,
        "convert.rs",
        "pub struct Visibility;\npub struct Widget;\nimpl From<Visibility> for Widget {\n    fn from(_: Visibility) -> Self { Widget }\n}\n",
    );
    write_source(
        root,
        "owner.rs",
        "pub struct FixturePolicy;\npub struct WidgetTree;\nimpl WidgetTree {\n    pub fn layout_fixture(&self) {}\n}\n",
    );
    write_source(
        root,
        "tests.rs",
        "#[test]\nfn fixture_renders() {}\n#[test]\nfn fixture_builder_parity() {}\nfn fixture_helper() {}\n#[test]\nfn fixture_renders_extended() {}\n",
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
fn validator_rejects_empty_records_and_missing_options() {
    let root = fixture_root("empty");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["records"] = Value::Array(Vec::new());
    let errors = errors_for(&ledger, &root);
    assert!(errors.contains(&LedgerError::EmptyRecords), "{errors:?}");

    // A ledger without the `visible` record misses discovered public API.
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["option"] == "visible")
        .unwrap();
    records.remove(position);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Visibility".to_owned(),
            option: "visible".to_owned(),
        }),
        "{errors:?}"
    );

    // A record for a removed option is stale.
    let mut ledger = fixture_ledger();
    ledger["records"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "symbol": "Visibility",
            "option": "removed_option",
            "default": false,
            "validation": "gone",
            "conversion": {
                "path": "crates/fixture/convert.rs",
                "kind": "trait_impl",
                "trait": "From",
                "target": "Widget",
                "source": "Visibility"
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
            symbol: "Visibility".to_owned(),
            option: "removed_option".to_owned(),
        }),
        "{errors:?}"
    );

    // A derived builder without an explicit builder_parity reference
    // fails, even when builder-named regressions exist elsewhere: names
    // alone prove nothing.
    let mut ledger = fixture_ledger();
    ledger.as_object_mut().unwrap().remove("builder_parity");
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::MissingBuilderCoverage {
            symbol: "Visibility".to_owned(),
        }),
        "{errors:?}"
    );

    // A builder_parity reference to a missing test fails like any other
    // regression reference.
    let mut ledger = fixture_ledger();
    ledger["builder_parity"]["Visibility"]["name"] =
        Value::String("no_such_parity_test".to_owned());
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::RegressionNotATest { reference, .. }
            if reference.contains("builder_parity::Visibility")
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn write_visibility(root: &Path, text: &str) {
    std::fs::create_dir_all(root.join("crates/incular-widgets/src/layout/basic")).unwrap();
    std::fs::write(
        root.join("crates/incular-widgets/src/layout/basic/visibility.rs"),
        text,
    )
    .unwrap();
}

const MINIATURE_VISIBILITY: &str = "#[derive(Clone, TypedBuilder)]\npub struct Visibility {\n    visible: bool,\n    child: Widget,\n}\npub struct Widget;\nimpl Visibility {\n    pub fn new(child: impl Into<Widget>) -> Self {\n        let _ = child;\n        unimplemented!()\n    }\n    pub fn visible(mut self, visible: bool) -> Self {\n        let _ = visible;\n        unimplemented!()\n    }\n}\n";

#[test]
fn validator_discovers_builder_only_fields_and_honors_skipped_setters() {
    // A builder-only field with no handwritten setter or constructor
    // parameter is still public API: it must fail with MissingOption.
    let root = fixture_root("builder-only");
    fixture_sources(&root);
    write_visibility(
        &root,
        &MINIATURE_VISIBILITY.replace(
            "    child: Widget,\n",
            "    child: Widget,\n    #[builder(default)]\n    extra: bool,\n",
        ),
    );
    let errors = errors_for(&fixture_ledger(), &root);
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Visibility".to_owned(),
            option: "extra".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);

    // A skipped setter generates no public API and needs no record.
    let root = fixture_root("skipped-setter");
    fixture_sources(&root);
    write_visibility(
        &root,
        &MINIATURE_VISIBILITY.replace(
            "    child: Widget,\n",
            "    child: Widget,\n    #[builder(default, setter(skip))]\n    internal_cache: bool,\n",
        ),
    );
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root, &FIXTURE_FAMILY) {
        panic!("skipped setters must not require ledger records: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_renaming_builder_attributes_and_invented_phases() {
    // A renaming setter cannot be mapped to a field name without macro
    // expansion: reject loudly instead of assuming coverage.
    let root = fixture_root("renamed-setter");
    fixture_sources(&root);
    write_visibility(
        &root,
        &MINIATURE_VISIBILITY.replace(
            "    child: Widget,\n",
            "    child: Widget,\n    #[builder(setter(prefix = \"with_\"))]\n    renamed: bool,\n",
        ),
    );
    let errors = errors_for(&fixture_ledger(), &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnsupportedBuilderAttribute { symbol, field, attribute }
            if symbol == "Visibility" && field == "renamed" && attribute.contains("prefix")
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);

    // The phase vocabulary is fixed in the validator: an unknown phase
    // fails even when also listed in the ledger's own allowed_phases.
    let root = fixture_root("invented-phase");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["allowed_phases"] = serde_json::json!(["layout", "teleport"]);
    ledger["records"][0]["consumers"] = serde_json::json!({
        "teleport": [
            {
                "path": "crates/fixture/owner.rs",
                "kind": "method",
                "owner": "WidgetTree",
                "name": "layout_fixture"
            }
        ]
    });
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnknownPhase { phase, .. } if phase == "teleport"
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_duplicate_options_and_unknown_phases() {
    let root = fixture_root("duplicates");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let duplicate = ledger["records"][0].clone();
    ledger["records"].as_array_mut().unwrap().push(duplicate);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::DuplicateOption {
            symbol: "Visibility".to_owned(),
            option: "visible".to_owned(),
        }),
        "{errors:?}"
    );

    let mut ledger = fixture_ledger();
    ledger["records"][0]["consumers"] = serde_json::json!({
        "teleport": [
            {
                "path": "crates/fixture/owner.rs",
                "kind": "method",
                "owner": "WidgetTree",
                "name": "layout_fixture"
            }
        ]
    });
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::UnknownPhase {
            record: "Visibility::visible".to_owned(),
            phase: "teleport".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_missing_methods_and_wrong_trait_targets() {
    let root = fixture_root("refs");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["records"][0]["consumers"]["layout"][0]["name"] =
        Value::String("no_such_method".to_owned());
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnresolvedReference { reason, .. }
            if reason.contains("no method 'no_such_method' on owner 'WidgetTree'")
        )),
        "{errors:?}"
    );

    // Same file and trait, but no impl targets Widget through Fixture.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["conversion"]["source"] = Value::String("Other".to_owned());
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnresolvedReference { reason, .. }
            if reason.contains("no source 'Other'")
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_comment_only_symbols_and_plain_helpers() {
    let root = fixture_root("comments");
    fixture_sources(&root);
    write_source(
        &root,
        "comments.rs",
        "// GhostPolicy documents a removed type.\nconst NOTE: &str = \"GhostPolicy\";\n",
    );
    let mut ledger = fixture_ledger();
    ledger["records"][0]["retained_owner"] = serde_json::json!({
        "path": "crates/fixture/comments.rs",
        "kind": "struct",
        "name": "GhostPolicy"
    });
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnresolvedReference { reason, .. }
            if reason.contains("only in comments or string literals")
        )),
        "{errors:?}"
    );

    // A helper without #[test] is not regression coverage.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["regressions"] = serde_json::json!([
        {"path": "crates/fixture/tests.rs", "name": "fixture_helper"}
    ]);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::RegressionNotATest { reason, .. }
            if reason.contains("carries no #[test] attribute")
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_prefix_only_test_matches_and_resolves_nested_modules() {
    let root = fixture_root("prefix");
    fixture_sources(&root);
    write_source(
        &root,
        "nested.rs",
        "mod inner {\n    #[test]\n    fn nested_works() {}\n}\n",
    );
    // fixture_renders_extended exists; fixture_renders must not match it.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["regressions"] = serde_json::json!([
        {"path": "crates/fixture/tests.rs", "name": "fixture_renders_missing"}
    ]);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::RegressionNotATest { reason, .. }
            if reason.contains("no exact match")
        )),
        "{errors:?}"
    );

    // Nested modules resolve through the module path.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["regressions"] = serde_json::json!([
        {"path": "crates/fixture/nested.rs", "name": "nested_works", "module": "inner"}
    ]);
    let nested_path = root.join("crates/fixture/nested.rs");
    assert!(nested_path.exists());
    if let Err(errors) = validate_ledger(&ledger, &root, &FIXTURE_FAMILY) {
        panic!("nested test must resolve: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
