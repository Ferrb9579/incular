//! Validate the baseline/aspect/fractional retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const BASELINE_ASPECT_FRACTIONAL_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "FractionallySizedBox",
            file: "crates/incular-widgets/src/layout/basic/baseline_aspect_fractional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "Baseline",
            file: "crates/incular-widgets/src/layout/basic/baseline_aspect_fractional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "AspectRatio",
            file: "crates/incular-widgets/src/layout/basic/baseline_aspect_fractional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "FittedBox",
            file: "crates/incular-widgets/src/layout/basic/baseline_aspect_fractional.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!(
        "../specs/baseline_aspect_fractional_properties.json"
    ))
    .unwrap()
}

#[test]
fn baseline_aspect_fractional_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &BASELINE_ASPECT_FRACTIONAL_FAMILY)
    {
        panic!(
            "baseline aspect fractional ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
