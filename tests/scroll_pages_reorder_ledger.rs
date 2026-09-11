//! Validate the PageView and animated/reorderable sliver ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SCROLL_PAGES_REORDER_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "PageView",
            file: "crates/incular-widgets/src/scrolling/scroll_views.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SliverAnimatedList",
            file: "crates/incular-widgets/src/scrolling/sliver_lists.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SliverAnimatedGrid",
            file: "crates/incular-widgets/src/advanced_slivers/animated.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SliverReorderableList",
            file: "crates/incular-widgets/src/scrolling/sliver_lists.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SliverAnimatedListController",
            file: "crates/incular-widgets/src/scrolling/sliver_lists.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SliverReorderController",
            file: "crates/incular-widgets/src/scrolling/sliver_lists.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!(
        "../specs/scroll_pages_reorder_properties.json"
    ))
    .unwrap()
}

#[test]
fn scroll_pages_reorder_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &SCROLL_PAGES_REORDER_FAMILY) {
        panic!(
            "scroll pages reorder ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
