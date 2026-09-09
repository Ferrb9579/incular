//! Validate the clip/opacity retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly, plus targeted negatives proving
//! both discovery styles (fluent method names for clips, constructor
//! parameters for opacity) neither miss options nor misclassify observer
//! getters as options.
mod ledger;

use ledger::{DiscoveryStyle, FamilySpec, LedgerError, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const CLIP_OPACITY_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "ClipRect",
            file: "crates/incular-widgets/src/layout/basic/clipping.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ClipRRect",
            file: "crates/incular-widgets/src/layout/basic/clipping.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ClipOval",
            file: "crates/incular-widgets/src/layout/basic/clipping.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ClipPath",
            file: "crates/incular-widgets/src/layout/basic/clipping.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Opacity",
            file: "crates/incular-widgets/src/tree/descriptors.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
        StructSpec {
            name: "FadeTransition",
            file: "crates/incular-widgets/src/tree/descriptors.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/clip_opacity_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn clip_opacity_ledger_validates() {
    if let Err(errors) = validate_ledger(&real_ledger(), &repository_root(), &CLIP_OPACITY_FAMILY) {
        panic!(
            "clip/opacity ledger invalid:\n{}",
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

const FIXTURE_API: &str = "pub struct Clip2d {\n    active: bool,\n}\nimpl Clip2d {\n    pub fn new(child: Widget) -> Self {\n        let _ = child;\n        unimplemented!()\n    }\n    pub fn behavior(mut self, behavior: u32) -> Self {\n        let _ = behavior;\n        unimplemented!()\n    }\n    pub fn is_active(&self) -> bool {\n        self.active\n    }\n}\npub struct Fade2d {\n    active: bool,\n}\nimpl Fade2d {\n    pub fn new(alpha: f32, child: Widget) -> Self {\n        let _ = (alpha, child);\n        unimplemented!()\n    }\n    pub fn alpha(&self) -> f32 {\n        0.0\n    }\n}\npub struct Widget;\n";

const FIXTURE_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Clip2d",
            file: "crates/fixture/src/api.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Fade2d",
            file: "crates/fixture/src/api.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
    ],
};

/// Minimal well-formed ledger over the fixture API: constructor parameters
/// plus the fluent option recorded, observer getters excluded.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "ClipFixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["compositor"],
        "records": [
            {
                "symbol": "Clip2d",
                "option": "child",
                "default": "required (new)",
                "validation": "widget parameter",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Clip2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Clip2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Clip2d",
                            "name": "behavior"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Clip2d",
                "option": "behavior",
                "default": "required (behavior)",
                "validation": "u32 tag",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Clip2d",
                    "name": "behavior"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Clip2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Clip2d",
                            "name": "behavior"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Fade2d",
                "option": "alpha",
                "default": "required (new)",
                "validation": "f32 alpha",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Fade2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Fade2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Fade2d",
                            "name": "new"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Fade2d",
                "option": "child",
                "default": "required, no default",
                "validation": "required widget parameter",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Fade2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Fade2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Fade2d",
                            "name": "new"
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
fn both_styles_cover_options_and_exclude_getters() {
    // The observer getters `is_active`/`alpha` are not configuration: the
    // well-formed fixture validates cleanly without records for them.
    let root = fixture_root("clip-positive");
    fixture_sources(&root);
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root, &FIXTURE_FAMILY) {
        panic!("well-formed clip ledger must validate: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);

    // A fluent option without a record fails with MissingOption.
    let root = fixture_root("clip-missing");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["option"] == "behavior")
        .unwrap();
    records.remove(position);
    let errors =
        validate_ledger(&ledger, &root, &FIXTURE_FAMILY).expect_err("fixture must be rejected");
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Clip2d".to_owned(),
            option: "behavior".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);

    // A constructor parameter without a record fails the same way.
    let root = fixture_root("clip-missing-ctor");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["symbol"] == "Fade2d" && record["option"] == "alpha")
        .unwrap();
    records.remove(position);
    let errors =
        validate_ledger(&ledger, &root, &FIXTURE_FAMILY).expect_err("fixture must be rejected");
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Fade2d".to_owned(),
            option: "alpha".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
