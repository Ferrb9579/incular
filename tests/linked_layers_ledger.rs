//! Validate the linked-layer retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly, plus targeted negatives proving
//! method-name discovery neither misses fluent options nor misclassifies
//! observer getters as options.
mod ledger;

use ledger::{DiscoveryStyle, FamilySpec, LedgerError, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const LINKED_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "CompositedTransformTarget",
            file: "crates/incular-widgets/src/compositing.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "CompositedTransformFollower",
            file: "crates/incular-widgets/src/compositing.rs",
            style: DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/linked_layers_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn linked_layers_ledger_validates() {
    if let Err(errors) = validate_ledger(&real_ledger(), &repository_root(), &LINKED_FAMILY) {
        panic!(
            "linked-layer ledger invalid:\n{}",
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

fn write_fixture_api(root: &Path, text: &str) {
    std::fs::create_dir_all(root.join("crates/fixture/src")).unwrap();
    std::fs::write(root.join("crates/fixture/src/api.rs"), text).unwrap();
}

const FIXTURE_API: &str = "pub struct Follower2d {\n    active: bool,\n}\nimpl Follower2d {\n    pub fn new(link: u32, child: Widget) -> Self {\n        let _ = (link, child);\n        unimplemented!()\n    }\n    pub fn offset(mut self, offset: Offset) -> Self {\n        let _ = offset;\n        unimplemented!()\n    }\n    pub fn link(&self) -> u32 {\n        0\n    }\n}\npub struct Widget;\npub struct Offset;\n";

const FIXTURE_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "Follower2d",
        file: "crates/fixture/src/api.rs",
        style: DiscoveryStyle::MethodNames,
    }],
};

/// Minimal well-formed ledger over the fixture API: constructor parameters
/// plus the fluent option recorded, the observer getter excluded.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "LinkedFixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["compositor"],
        "records": [
            {
                "symbol": "Follower2d",
                "option": "link",
                "default": "required (new)",
                "validation": "u32 tag",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Follower2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Follower2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Follower2d",
                            "name": "offset"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Follower2d",
                "option": "child",
                "default": "required, no default",
                "validation": "required widget parameter",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Follower2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Follower2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Follower2d",
                            "name": "offset"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Follower2d",
                "option": "offset",
                "default": "required (offset)",
                "validation": "2d offset",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Follower2d",
                    "name": "offset"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Follower2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Follower2d",
                            "name": "offset"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            }
        ]
    })
}

fn fixture_sources(root: &Path) {
    write_fixture_api(root, FIXTURE_API);
    std::fs::write(
        root.join("crates/fixture/tests.rs"),
        "#[test]\nfn fixture_moves() {}\n",
    )
    .unwrap();
}

#[test]
fn method_names_cover_fluent_options_and_exclude_getters() {
    // The observer getter `link` is not configuration: the well-formed
    // fixture validates cleanly without a record for it.
    let root = fixture_root("linked-positive");
    fixture_sources(&root);
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root, &FIXTURE_FAMILY) {
        panic!("well-formed linked ledger must validate: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);

    // A fluent option without a record fails with MissingOption.
    let root = fixture_root("linked-missing");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["option"] == "offset")
        .unwrap();
    records.remove(position);
    let errors =
        validate_ledger(&ledger, &root, &FIXTURE_FAMILY).expect_err("fixture must be rejected");
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Follower2d".to_owned(),
            option: "offset".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
