//! Validate the SafeArea/SplitView/OverflowBar retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const UTILITY_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "SafeArea",
            file: "crates/incular-widgets/src/safe_area.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SplitView",
            file: "crates/incular-widgets/src/layout/basic/split_view.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "OverflowBar",
            file: "crates/incular-widgets/src/layout/basic/utility.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/utility_properties.json")).unwrap()
}

#[test]
fn utility_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &UTILITY_FAMILY) {
        panic!(
            "utility ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
