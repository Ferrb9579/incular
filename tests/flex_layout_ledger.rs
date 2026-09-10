//! Validate the Row/Column/Flex/Flexible/Expanded/Spacer ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const FLEX_LAYOUT_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Row",
            file: "crates/incular-widgets/src/layout/flex.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Column",
            file: "crates/incular-widgets/src/layout/flex.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Flex",
            file: "crates/incular-widgets/src/layout/flex.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Flexible",
            file: "crates/incular-widgets/src/layout/flex.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Expanded",
            file: "crates/incular-widgets/src/layout/flex.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Spacer",
            file: "crates/incular-widgets/src/layout/flex.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/flex_layout_properties.json")).unwrap()
}

#[test]
fn flex_layout_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &FLEX_LAYOUT_FAMILY) {
        panic!(
            "flex layout ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
