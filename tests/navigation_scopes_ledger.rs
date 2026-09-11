//! Validate the navigation-scope ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const NAVIGATION_SCOPES_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "PopAttempt",
            file: "crates/incular-widgets/src/navigation/back_dispatch.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "BackHandlerResult",
            file: "crates/incular-widgets/src/navigation/back_dispatch.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "BackButtonDispatcher",
            file: "crates/incular-widgets/src/navigation/back_dispatch.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "AnimatedModalBarrier",
            file: "crates/incular-widgets/src/navigation/barrier.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PageStorageKey",
            file: "crates/incular-widgets/src/navigation/page_storage.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PageStorageIdentifier",
            file: "crates/incular-widgets/src/navigation/page_storage.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PageStorage",
            file: "crates/incular-widgets/src/navigation/page_storage.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PopScope",
            file: "crates/incular-widgets/src/navigation/pop_scopes.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "NavigatorPopHandler",
            file: "crates/incular-widgets/src/navigation/pop_scopes.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "BackButtonListener",
            file: "crates/incular-widgets/src/navigation/pop_scopes.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "RootRestorationScope",
            file: "crates/incular-widgets/src/navigation/restoration.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "UnmanagedRestorationScope",
            file: "crates/incular-widgets/src/navigation/restoration.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/navigation_scopes_properties.json")).unwrap()
}

#[test]
fn navigation_scopes_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &NAVIGATION_SCOPES_FAMILY) {
        panic!(
            "navigation scopes ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
