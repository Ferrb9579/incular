//! Validate the image-descriptor retained-property ledger.
//!
//! Family-specific checks over the shared [`ledger`] validator module:
//! the real ledger must validate cleanly.
mod ledger;

use ledger::{FamilySpec, StructSpec, validate_ledger};
use serde_json::Value;
use std::path::Path;

const IMAGE_FAMILY: FamilySpec = FamilySpec {
    structs: &[
        StructSpec {
            name: "Image",
            file: "crates/incular-widgets/src/tree/descriptors.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "RawImage",
            file: "crates/incular-widgets/src/painting_effects/images.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
        StructSpec {
            name: "ImageIcon",
            file: "crates/incular-widgets/src/painting_effects/images.rs",
            style: ledger::DiscoveryStyle::MethodNames,
        },
    ],
};

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/image_properties.json")).unwrap()
}

#[test]
fn image_ledger_validates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
    if let Err(errors) = validate_ledger(&real_ledger(), &root, &IMAGE_FAMILY) {
        panic!(
            "image ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
