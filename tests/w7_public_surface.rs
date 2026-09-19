#[path = "ledger/w7.rs"]
mod w7;
use std::path::Path;
use w7::{assert_snapshot, root_surface_snapshot};

#[test]
fn w7_public_surface_matches_source_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = root_surface_snapshot(
        root,
        &[
            ("incular_controls", "crates/incular-controls/src/lib.rs"),
            ("incular_material", "crates/incular-material/src/lib.rs"),
        ],
    );
    assert_snapshot(
        "w7_public_surface.json",
        &actual,
        include_str!("../specs/w7_public_surface.json"),
        root,
    );
}
