//! Validate the pointer-blocking retained-property ledger against the tree.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const POINTER_BLOCKING_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "IgnorePointer",
            file: "crates/incular-widgets/src/gestures.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "AbsorbPointer",
            file: "crates/incular-widgets/src/gestures.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/pointer_blocking_properties.json")).unwrap()
}

#[test]
fn pointer_blocking_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &POINTER_BLOCKING_FAMILY) {
        panic!(
            "pointer blocking ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
