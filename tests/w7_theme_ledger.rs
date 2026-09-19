#[path = "ledger/w7.rs"]
mod w7;
use serde_json::json;
use std::path::Path;
use w7::{assert_snapshot, filtered_snapshot};

#[test]
fn w7_theme_options_match_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let controls = filtered_snapshot(
        root,
        "incular_controls",
        "crates/incular-controls/src",
        |path| {
            path.file_name()
                .is_some_and(|name| name == "theme.rs" || name == "styles.rs")
        },
    );
    let material = filtered_snapshot(
        root,
        "incular_material",
        "crates/incular-material/src",
        |path| {
            let text = path.to_string_lossy().replace('\\', "/");
            text.contains("/theme/")
                || text.ends_with("/theme.rs")
                || text.ends_with("/feedback/progress.rs")
        },
    );
    let actual = json!({"schema_version": 3, "controls": controls, "material": material});
    assert_snapshot(
        "w7_theme_properties.json",
        &actual,
        include_str!("../specs/w7_theme_properties.json"),
        root,
    );
}
