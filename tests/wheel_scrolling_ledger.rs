//! Validate the wheel-scrolling ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const WHEEL_SCROLLING_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "WheelChildDelegate",
            file: "crates/incular-widgets/src/advanced_scrolling/wheel.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "WheelMatrix",
            file: "crates/incular-widgets/src/advanced_scrolling/wheel.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "WheelProjection",
            file: "crates/incular-widgets/src/advanced_scrolling/wheel.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ListWheelViewport",
            file: "crates/incular-widgets/src/advanced_scrolling/wheel.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ListWheelScrollView",
            file: "crates/incular-widgets/src/advanced_scrolling/wheel.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "FixedExtentScrollController",
            file: "crates/incular-widgets/src/advanced_scrolling/wheel.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/wheel_scrolling_properties.json")).unwrap()
}

#[test]
fn wheel_scrolling_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &WHEEL_SCROLLING_FAMILY) {
        panic!(
            "wheel scrolling ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
