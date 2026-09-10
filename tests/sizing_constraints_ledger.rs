//! Validate the sizing/constraint retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const SIZING_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "SizedBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ConstrainedBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "LimitedBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "OverflowBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "UnconstrainedBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "LayoutBuilder",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "IntrinsicWidth",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "IntrinsicHeight",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "SizedOverflowBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ConstraintsTransformBox",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PreferredSize",
            file: "crates/incular-widgets/src/layout/basic/sizing_constraints.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/sizing_constraints_properties.json")).unwrap()
}

#[test]
fn sizing_constraints_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &SIZING_FAMILY) {
        panic!(
            "sizing constraints ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
