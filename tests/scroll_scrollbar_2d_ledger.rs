//! Validate the scrollbar and two-dimensional scrolling ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SCROLL_SCROLLBAR_2D_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "RawScrollbar",
            file: "crates/incular-widgets/src/advanced_scrolling/raw_scrollbar.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "TwoDimensionalChildDelegate",
            file: "crates/incular-widgets/src/advanced_scrolling/two_dimensional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "TwoDimensionalScrollable",
            file: "crates/incular-widgets/src/advanced_scrolling/two_dimensional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "TwoDimensionalConstraints",
            file: "crates/incular-widgets/src/advanced_scrolling/two_dimensional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "TwoDimensionalViewport",
            file: "crates/incular-widgets/src/advanced_scrolling/two_dimensional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "TwoDimensionalScrollView",
            file: "crates/incular-widgets/src/advanced_scrolling/two_dimensional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ChildVicinity",
            file: "crates/incular-widgets/src/advanced_scrolling/two_dimensional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/scroll_scrollbar_2d_properties.json")).unwrap()
}

#[test]
fn scroll_scrollbar_2d_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &SCROLL_SCROLLBAR_2D_FAMILY) {
        panic!(
            "scroll scrollbar 2d ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
