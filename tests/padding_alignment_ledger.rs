//! Validate the Padding/Align/Center retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const PADDING_ALIGNMENT_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Padding",
            file: "crates/incular-widgets/src/layout/basic/padding_alignment.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Align",
            file: "crates/incular-widgets/src/layout/basic/padding_alignment.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Center",
            file: "crates/incular-widgets/src/layout/basic/padding_alignment.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/padding_alignment_properties.json")).unwrap()
}

#[test]
fn padding_alignment_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &PADDING_ALIGNMENT_FAMILY) {
        panic!(
            "padding alignment ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
