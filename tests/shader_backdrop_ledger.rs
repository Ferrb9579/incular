//! Validate the shader/backdrop retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly, plus targeted negatives proving
//! constructor-parameter discovery neither misses parameters nor
//! misclassifies observer getters as options.
mod ledger;

use ledger::{DiscoveryStyle, FamilySpec, LedgerError, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SHADER_BACKDROP_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "ShaderCallback",
            file: "crates/incular-widgets/src/compositing.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
        StructSpec {
            name: "ShaderMask",
            file: "crates/incular-widgets/src/compositing.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
        StructSpec {
            name: "BackdropFilter",
            file: "crates/incular-widgets/src/compositing.rs",
            style: DiscoveryStyle::ConstructorParams,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/shader_backdrop_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn shader_backdrop_ledger_validates() {
    if let Err(errors) =
        validate_ledger(&real_ledger(), &repository_root(), &SHADER_BACKDROP_FAMILY)
    {
        panic!(
            "shader/backdrop ledger invalid:\n{}",
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

const FIXTURE_API: &str = "pub struct Mask2d {\n    active: bool,\n}\nimpl Mask2d {\n    pub fn new(shader: u32, child: Widget) -> Self {\n        let _ = (shader, child);\n        unimplemented!()\n    }\n    pub fn shader(mut self, shader: u32) -> Self {\n        let _ = shader;\n        unimplemented!()\n    }\n    pub fn is_active(&self) -> bool {\n        self.active\n    }\n}\npub struct Widget;\n";

const FIXTURE_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "Mask2d",
        file: "crates/fixture/src/api.rs",
        style: DiscoveryStyle::ConstructorParams,
    }],
};

/// Minimal well-formed ledger over the fixture API: constructor parameters
/// plus the fluent option recorded, the observer getter excluded.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "MaskFixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["compositor"],
        "records": [
            {
                "symbol": "Mask2d",
                "option": "shader",
                "default": "required (new)",
                "validation": "u32 tag",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Mask2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Mask2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Mask2d",
                            "name": "shader"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_moves"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Mask2d",
                "option": "child",
                "default": "required, no default",
                "validation": "required widget parameter",
                "conversion": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "method",
                    "owner": "Mask2d",
                    "name": "new"
                },
                "retained_owner": {
                    "path": "crates/fixture/src/api.rs",
                    "kind": "struct",
                    "name": "Mask2d"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "compositor": [
                        {
                            "path": "crates/fixture/src/api.rs",
                            "kind": "method",
                            "owner": "Mask2d",
                            "name": "shader"
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
fn constructor_params_cover_fluent_options_and_exclude_getters() {
    // The observer getter `is_active` is not configuration: the
    // well-formed fixture validates cleanly without a record for it.
    let root = fixture_root("mask-positive");
    fixture_sources(&root);
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root, &FIXTURE_FAMILY) {
        panic!("well-formed mask ledger must validate: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);

    // A fluent option without a record fails with MissingOption.
    let root = fixture_root("mask-missing");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["option"] == "shader")
        .unwrap();
    records.remove(position);
    let errors =
        validate_ledger(&ledger, &root, &FIXTURE_FAMILY).expect_err("fixture must be rejected");
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Mask2d".to_owned(),
            option: "shader".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
