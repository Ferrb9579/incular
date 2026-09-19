#[path = "ledger/w7.rs"]
mod w7;
use std::path::Path;
use w7::{assert_snapshot, crate_snapshot};

#[test]
fn w7_controls_public_options_match_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = crate_snapshot(root, "incular_controls", "crates/incular-controls/src");
    assert_snapshot(
        "w7_controls_properties.json",
        &actual,
        include_str!("../specs/w7_controls_properties.json"),
        root,
    );
}
