//! Source-level guardrails for the Widgets/controls/Material boundary.

use std::{collections::HashSet, fs, path::Path};

#[test]
fn widgets_root_has_no_globbed_retained_or_layout_surface() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/src/lib.rs"),
    )
    .unwrap();
    assert!(!source.contains("pub use tree::*"));
    assert!(!source.contains("pub use layout::*"));
    assert!(source.contains("pub mod internal"));
    assert!(
        source.contains("#[doc(hidden)]\npub mod devtools"),
        "target-side DevTools is an internal bridge, not Widgets vocabulary"
    );
    assert!(source.contains("pub(crate) mod devtools_props"));
}

#[test]
fn widgets_crate_does_not_depend_on_material() {
    let manifest = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/Cargo.toml"),
    )
    .unwrap();
    assert!(!manifest.contains("incular-material"));
}

#[test]
fn facade_prelude_does_not_import_material_implicitly() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular/src/lib.rs"))
            .unwrap();
    let prelude = source
        .split("pub mod prelude {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("facade prelude module");
    assert!(!prelude.contains("incular_material::prelude"));
    assert!(source.contains("pub mod material_prelude"));
}

#[test]
fn incular_only_extensions_are_not_root_widgets_exports() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/src/lib.rs"),
    )
    .unwrap();
    let public_lines = source
        .lines()
        .filter(|line| line.contains("pub use ") && !line.contains("pub(crate)"))
        .collect::<Vec<_>>();
    for forbidden in [
        "SplitView",
        "SplitPosition",
        "PathView",
        "Blur",
        "DropShadow",
        "Effects",
        "WidgetTree",
        "WidgetKind",
        "RenderObject",
        "RawButton",
        "SliverAppBar",
    ] {
        assert!(
            !public_lines.iter().any(|line| line.contains(forbidden)),
            "Inc﻿ular extension leaked into public Widgets exports: {forbidden}"
        );
    }
}

#[test]
fn material_and_widgets_do_not_duplicate_canonical_primitives() {
    let widgets = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/src/lib.rs"),
    )
    .unwrap();
    let material = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-material/src/lib.rs"),
    )
    .unwrap();

    // Material wraps the Widgets primitive; it must not define another editor,
    // navigator, scroll controller, or semantics tree under the same name.
    for duplicate in [
        "pub struct EditableText",
        "pub struct Navigator",
        "pub struct ScrollController",
        "pub struct Semantics",
    ] {
        assert!(
            !material.contains(duplicate),
            "Material duplicated a Widgets primitive: {duplicate}"
        );
    }

    // Styled entry points must stay out of the renderer-neutral root, even
    // when the facade has an optional Material feature enabled.
    for material_only in [
        "RawMaterialButton",
        "ElevatedButton",
        "FilledButton",
        "TextField",
        "TextFormField",
    ] {
        let public_lines = widgets
            .lines()
            .filter(|line| line.contains("pub use ") && !line.contains("pub(crate)"));
        assert!(
            !public_lines
                .clone()
                .any(|line| line.contains(material_only)),
            "Material symbol leaked into Widgets root: {material_only}"
        );
    }

    let material_sources = [
        "crates/incular-material/src/foundation.rs",
        "crates/incular-material/src/foundation/surfaces.rs",
        "crates/incular-material/src/components.rs",
    ];
    let material_struct_count = material_sources
        .iter()
        .map(|path| {
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap()
                .matches("pub struct Material")
                .count()
        })
        .sum::<usize>();
    assert_eq!(
        material_struct_count, 1,
        "Material surface must have one implementation; components should reuse foundation"
    );
}

#[test]
fn every_widgets_root_reexport_is_in_the_pinned_flutter_graph() {
    let root = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/src/lib.rs"),
    )
    .unwrap();
    let graph = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/flutter_widgets_3471_parity.jsonl"),
    )
    .unwrap()
    .lines()
    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .filter_map(|row| row["flutter_symbol"].as_str().map(str::to_owned))
    .collect::<HashSet<_>>();

    let mut root_names = HashSet::new();
    for statement in root
        .split("pub use ")
        .skip(1)
        .filter_map(|tail| tail.split_once(';').map(|(statement, _)| statement))
    {
        if statement.contains("pub(crate)") {
            continue;
        }
        let candidates = statement
            .rsplit_once('{')
            .and_then(|(_, body)| body.rsplit_once('}').map(|(_, body)| body))
            .map_or_else(
                || vec![statement.rsplit("::").next().unwrap_or(statement)],
                |body| body.split(',').collect::<Vec<_>>(),
            );
        for candidate in candidates {
            let name = candidate.split(" as ").next().unwrap_or(candidate).trim();
            if !name.is_empty() && name != "self" {
                root_names.insert(name.to_owned());
            }
        }
    }

    for name in root_names {
        assert!(
            graph.contains(&name),
            "non-Flutter symbol leaked into incular-widgets root: {name}"
        );
    }
}

#[test]
fn task23_dart_mechanics_are_not_public_widgets_api() {
    let widgets = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/src/lib.rs"),
    )
    .unwrap();
    let facade =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular/src/lib.rs"))
            .unwrap();
    let forbidden = [
        "ChangeNotifier",
        "ValueNotifier",
        "ValueListenable",
        "ListenableBuilder",
        "ValueListenableBuilder",
        "FutureBuilder",
        "StreamBuilder",
        "StatefulBuilder",
        "StatefulWidget",
        "StatelessWidget",
        "GlobalKey",
        "GlobalObjectKey",
        "TickerProviderStateMixin",
        "SingleTickerProviderStateMixin",
        "AutomaticKeepAliveClientMixin",
        "RestorationMixin",
    ];
    for symbol in forbidden {
        assert!(
            !widgets
                .lines()
                .filter(|line| line.contains("pub use ") && !line.contains("pub(crate)"))
                .any(|line| line.contains(symbol)),
            "Dart mechanic leaked from incular-widgets root: {symbol}"
        );
        let prelude = facade
            .split("pub mod prelude {")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .expect("facade prelude module");
        assert!(
            !prelude.lines().any(|line| line.contains(symbol)),
            "Dart mechanic leaked from incular::prelude: {symbol}"
        );
    }
}
