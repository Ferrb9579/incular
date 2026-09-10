//! Validate the Wrap/Table retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const COLLECTIONS_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Wrap",
            file: "crates/incular-widgets/src/layout/wrap.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Table",
            file: "crates/incular-widgets/src/layout/table.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/collections_properties.json")).unwrap()
}

#[test]
fn collections_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &COLLECTIONS_FAMILY) {
        panic!(
            "collections ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
