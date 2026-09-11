//! Validate the viewport-configuration and ordinary-list-scrolling ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SCROLL_VIEWPORT_LISTS_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Scrollable",
            file: "crates/incular-widgets/src/scrolling/sliver_descriptors.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Viewport",
            file: "crates/incular-widgets/src/scrolling/sliver_descriptors.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "CustomScrollView",
            file: "crates/incular-widgets/src/scrolling/sliver_descriptors.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ListView",
            file: "crates/incular-widgets/src/scrolling/scroll_views.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SliverList",
            file: "crates/incular-widgets/src/scrolling/sliver_descriptors.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ScrollController",
            file: "crates/incular-scroll/src/controller.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!(
        "../specs/scroll_viewport_lists_properties.json"
    ))
    .unwrap()
}

#[test]
fn scroll_viewport_lists_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &SCROLL_VIEWPORT_LISTS_FAMILY) {
        panic!(
            "scroll viewport lists ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
