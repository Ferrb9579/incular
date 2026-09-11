//! Validate the scroll-physics ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SCROLL_PHYSICS_FAMILY: FamilySpec = FamilySpec {
    structs: &[StructSpec {
        name: "ScrollPhysics",
        file: "crates/incular-scroll/src/physics.rs",
        style: ledger::DiscoveryStyle::MethodNames,
    }],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/scroll_physics_properties.json")).unwrap()
}

#[test]
fn scroll_physics_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &SCROLL_PHYSICS_FAMILY) {
        panic!(
            "scroll physics ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
