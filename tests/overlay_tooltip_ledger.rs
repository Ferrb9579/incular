//! Validate the overlay-portal and tooltip lifecycle ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const OVERLAY_TOOLTIP_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "OverlayPortal",
            file: "crates/incular-widgets/src/navigation_scopes.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "RawTooltip",
            file: "crates/incular-widgets/src/raw_tooltip.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "RawTooltipController",
            file: "crates/incular-widgets/src/raw_tooltip.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/overlay_tooltip_properties.json")).unwrap()
}

#[test]
fn overlay_tooltip_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &OVERLAY_TOOLTIP_FAMILY) {
        panic!(
            "overlay tooltip ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
