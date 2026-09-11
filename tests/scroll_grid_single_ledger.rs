//! Validate the grid, single-child, and animated-list scrolling ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SCROLL_GRID_SINGLE_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "SingleChildScrollView",
            file: "crates/incular-widgets/src/scrolling/scroll_views.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "GridView",
            file: "crates/incular-widgets/src/scrolling/scroll_views.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "AnimatedList",
            file: "crates/incular-widgets/src/advanced_slivers/animated.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "AnimatedGrid",
            file: "crates/incular-widgets/src/advanced_slivers/animated.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "AnimatedCollectionController",
            file: "crates/incular-widgets/src/advanced_slivers/animated.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/scroll_grid_single_properties.json")).unwrap()
}

#[test]
fn scroll_grid_single_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &SCROLL_GRID_SINGLE_FAMILY) {
        panic!(
            "scroll grid single ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
