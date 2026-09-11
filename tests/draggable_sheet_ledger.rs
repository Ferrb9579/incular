//! Validate the draggable-sheet ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const DRAGGABLE_SHEET_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "DraggableSheetExtent",
            file: "crates/incular-widgets/src/advanced_scrolling/draggable.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "DraggableScrollableSheet",
            file: "crates/incular-widgets/src/advanced_scrolling/draggable.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "DraggableScrollableState",
            file: "crates/incular-widgets/src/advanced_scrolling/draggable.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/draggable_sheet_properties.json")).unwrap()
}

#[test]
fn draggable_sheet_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &DRAGGABLE_SHEET_FAMILY) {
        panic!(
            "draggable sheet ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
