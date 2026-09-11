//! Validate the platform-wrapper ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const PLATFORM_WRAPPERS_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "MenuItemId",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PlatformMenuShortcut",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PlatformMenuItem",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PlatformMenuItemGroup",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PlatformMenu",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PlatformMenuBarController",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "PlatformMenuBar",
            file: "crates/incular-widgets/src/platform_widgets/menu.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "WindowDragRegion",
            file: "crates/incular-widgets/src/app_shell/window_chrome.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "WindowResizeRegion",
            file: "crates/incular-widgets/src/app_shell/window_chrome.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/platform_wrappers_properties.json")).unwrap()
}

#[test]
fn platform_wrappers_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &PLATFORM_WRAPPERS_FAMILY) {
        panic!(
            "platform wrappers ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
