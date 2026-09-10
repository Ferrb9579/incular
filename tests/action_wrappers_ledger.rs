//! Validate the shortcut/action-wrapper retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const ACTION_WRAPPERS_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "CallbackShortcuts",
            file: "crates/incular-widgets/src/focus_keyboard.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ActionListener",
            file: "crates/incular-widgets/src/focus_keyboard.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "FocusableActionDetector",
            file: "crates/incular-widgets/src/focus_keyboard.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/action_wrappers_properties.json")).unwrap()
}

#[test]
fn action_wrappers_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &ACTION_WRAPPERS_FAMILY) {
        panic!(
            "action wrappers ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
