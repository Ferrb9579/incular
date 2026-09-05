//! Source-level guardrails for the Widgets/controls/Material boundary.

use std::{collections::HashSet, fs, path::Path};

fn rust_sources_under(root: &Path) -> String {
    let mut pending = vec![root.to_path_buf()];
    let mut source = String::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                source.push_str(&fs::read_to_string(path).unwrap());
                source.push('\n');
            }
        }
    }
    source
}

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
fn widget_transport_keeps_retained_taxonomy_private() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let specs = fs::read_to_string(root.join("crates/incular-widgets/src/tree/specs.rs")).unwrap();
    let widget = rust_sources_under(&root.join("crates/incular-widgets/src/tree/widget"));
    let internal = fs::read_to_string(root.join("crates/incular-widgets/src/internal.rs")).unwrap();

    assert!(specs.contains("pub(crate) enum WidgetKind"));
    assert!(!specs.contains("pub enum WidgetKind"));
    assert!(!internal.contains("pub use crate::tree::*"));
    assert!(!widget.contains("impl Deref for Widget"));
    assert!(!widget.contains("impl DerefMut for Widget"));

    for removed in [
        "pub fn row(",
        "pub fn column(",
        "pub fn stack(",
        "pub fn padding(",
        "pub fn text(",
        "pub fn visibility(",
        "pub fn aspect_ratio(",
        "pub fn fixed_box(",
    ] {
        assert!(
            !widget.contains(removed),
            "legacy Widget constructor leaked: {removed}"
        );
    }
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
fn retained_and_styled_implementations_are_not_root_widgets_exports() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-widgets/src/lib.rs"),
    )
    .unwrap();
    let exports = root_reexports(&source);
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
            !exports.contains(forbidden),
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

    let material_sources = rust_sources_under(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/incular-material/src"),
    );
    let material_struct_count = material_sources.matches("pub struct Material {").count();
    assert_eq!(
        material_struct_count, 1,
        "Material surface must have one implementation; components should reuse foundation"
    );
}

#[test]
fn every_widgets_root_reexport_has_a_reference_or_incular_decision() {
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

    let root_names = root_reexports(&root);
    let extensions: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../specs/widgets_api_extensions.json")).unwrap();
    let mut reviewed = HashSet::new();
    for extension in &extensions {
        let symbols = extension["symbols"].as_array().unwrap();
        assert!(!symbols.is_empty());
        for symbol in symbols {
            let symbol = symbol.as_str().unwrap();
            assert!(reviewed.insert(symbol), "duplicate extension {symbol}");
            assert!(root_names.contains(symbol), "stale extension {symbol}");
            assert!(
                !graph.contains(symbol),
                "extension duplicates reference {symbol}"
            );
        }
        assert_eq!(extension["owner"], "incular-widgets");
        assert!(extension["reason"].as_str().unwrap().len() > 20);
        assert!(!extension["decision"].as_str().unwrap().is_empty());
        let evidence = extension["evidence"].as_str().unwrap();
        assert!(evidence.starts_with("tests/") || evidence.contains("/tests/"));
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(evidence)
                .is_file()
        );
    }
    for name in root_names {
        assert!(
            graph.contains(&name) || reviewed.contains(name.as_str()),
            "Widgets export needs a reference or reviewed Incular decision: {name}"
        );
    }
}

fn root_reexports(source: &str) -> HashSet<String> {
    fn names(tree: &syn::UseTree, output: &mut HashSet<String>) {
        match tree {
            syn::UseTree::Path(path) => names(&path.tree, output),
            syn::UseTree::Name(name) => {
                output.insert(name.ident.to_string());
            }
            syn::UseTree::Rename(rename) => {
                output.insert(rename.rename.to_string());
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    names(item, output);
                }
            }
            syn::UseTree::Glob(_) => panic!("Widgets root exports must be explicit"),
        }
    }
    let mut output = HashSet::new();
    for item in syn::parse_file(source).unwrap().items {
        if let syn::Item::Use(item) = item
            && matches!(item.vis, syn::Visibility::Public(_))
        {
            names(&item.tree, &mut output);
        }
    }
    output
}

#[test]
fn export_inventory_uses_rust_visibility_and_aliases() {
    let exports = root_reexports(
        "// pub use fake::Comment;\n pub(crate) use hidden::Bridge;\n pub use domain::{Original as Alias, nested::{One, Two}};",
    );
    assert_eq!(
        exports,
        ["Alias", "One", "Two"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
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
