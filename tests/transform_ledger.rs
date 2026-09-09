//! Validate the Transform retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly, plus targeted negatives proving
//! constructor-parameter discovery neither misses parameters nor
//! misclassifies getters as options.
mod ledger;

use ledger::{DiscoveryStyle, FamilySpec, LedgerError, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const TRANSFORM_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Transform",
            file: "crates/incular-widgets/src/tree/descriptors.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
        StructSpec {
            name: "FractionalTranslation",
            file: "crates/incular-widgets/src/layout/basic/transforms.rs",
            style: DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "RotatedBox",
            file: "crates/incular-widgets/src/layout/basic/transforms.rs",
            style: DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/transform_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn transform_ledger_validates() {
    if let Err(errors) = validate_ledger(&real_ledger(), &repository_root(), &TRANSFORM_FAMILY) {
        panic!(
            "transform ledger invalid:\n{}",
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

const FIXTURE_API: &str = "pub struct Transform2d {\n    active: bool,\n}\nimpl Transform2d {\n    pub fn new(kind: u32, child: Widget) -> Self {\n        let _ = (kind, child);\n        unimplemented!()\n    }\n    pub fn move_by(mut self, offset: Offset) -> Self {\n        let _ = offset;\n        unimplemented!()\n    }\n    pub fn label(&self) -> String {\n        String::new()\n    }\n}\npub struct Widget;\npub struct Offset;\n";

const FIXTURE_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "Transform2d",
        file: "crates/fixture/src/api.rs",
        style: DiscoveryStyle::ConstructorParams,
    }],
};

/// Minimal well-formed ledger over the fixture API: every constructor
/// parameter recorded, the observer getter excluded.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "TransformFixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["compositor"],
        "records": [
            {
                "symbol": "Transform2d",
                "option": "kind",
                "default": "required (new)",
                "validation": "u32 tag",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Transform2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Transform2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Transform2d",
                            "name": "move_by"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Transform2d",
                "option": "child",
                "default": "required, no default",
                "validation": "required widget parameter",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Transform2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Transform2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Transform2d",
                            "name": "move_by"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Transform2d",
                "option": "offset",
                "default": "required (move_by)",
                "validation": "2d offset",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Transform2d",
                    "name": "move_by"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Transform2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Transform2d",
                            "name": "move_by"
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
fn constructor_params_cover_parameters_and_exclude_getters() {
    // The observer getter `label` is not configuration: the well-formed
    // fixture validates cleanly without a record for it.
    let root = fixture_root("ctor-positive");
    fixture_sources(&root);
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root, &FIXTURE_FAMILY) {
        panic!("well-formed constructor ledger must validate: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);

    // A constructor parameter without a record fails with MissingOption.
    let root = fixture_root("ctor-missing");
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
            symbol: "Transform2d".to_owned(),
            option: "offset".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
