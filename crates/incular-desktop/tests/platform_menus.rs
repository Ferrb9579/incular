use incular_desktop::{
    NativeMenuCommandRegistry, NativeMenuRegistryError, matching_menu_shortcut,
    native_menu_structure_equal,
};
use incular_widgets::{
    MenuItemId, PlatformMenuShortcut, PlatformMenuSnapshot, PlatformMenuSnapshotNode,
    ShortcutModifiers,
};

fn leaf(id: &str, label: &str) -> PlatformMenuSnapshotNode {
    PlatformMenuSnapshotNode {
        id: id.into(),
        label: label.to_owned(),
        tooltip: None,
        shortcut: None,
        enabled: true,
        selectable: true,
        separator: false,
        children: Vec::new(),
    }
}

fn snapshot(nodes: Vec<PlatformMenuSnapshotNode>) -> PlatformMenuSnapshot {
    PlatformMenuSnapshot { menus: nodes }
}

#[test]
fn stable_ids_roundtrip_and_removed_commands_are_never_reused() {
    let mut registry = NativeMenuCommandRegistry::with_max_command_id(32);
    registry
        .synchronize(&snapshot(vec![leaf("a", "A"), leaf("b", "B")]))
        .unwrap();
    let a = registry.command_for(&"a".into()).unwrap();
    let b = registry.command_for(&"b".into()).unwrap();
    assert_eq!(registry.resolve(a), Some(&MenuItemId::from("a")));

    registry
        .synchronize(&snapshot(vec![leaf("a", "Renamed"), leaf("c", "C")]))
        .unwrap();
    let c = registry.command_for(&"c".into()).unwrap();
    assert_eq!(registry.command_for(&"a".into()), Some(a));
    assert_eq!(registry.resolve(b), None, "removed command is stale");
    assert_ne!(c, b, "retired native commands are never reused");
}

#[test]
fn removed_then_readded_id_receives_a_fresh_command() {
    let mut registry = NativeMenuCommandRegistry::with_max_command_id(32);
    registry
        .synchronize(&snapshot(vec![leaf("a", "A")]))
        .unwrap();
    let first = registry.command_for(&"a".into()).unwrap();
    registry.synchronize(&snapshot(Vec::new())).unwrap();
    registry
        .synchronize(&snapshot(vec![leaf("a", "A")]))
        .unwrap();
    assert_ne!(registry.command_for(&"a".into()), Some(first));
    assert_eq!(registry.resolve(first), None);
}

#[test]
fn duplicate_snapshot_rejects_without_mutating_registry() {
    let mut registry = NativeMenuCommandRegistry::with_max_command_id(32);
    registry
        .synchronize(&snapshot(vec![leaf("a", "A")]))
        .unwrap();
    let generation = registry.generation();
    let command = registry.command_for(&"a".into());
    let duplicate = snapshot(vec![leaf("dup", "A"), leaf("dup", "B")]);
    assert_eq!(
        registry.synchronize(&duplicate),
        Err(NativeMenuRegistryError::DuplicateId("dup".into()))
    );
    assert_eq!(registry.generation(), generation);
    assert_eq!(registry.command_for(&"a".into()), command);
}

#[test]
fn shortcut_matching_respects_enabled_state_and_exact_modifiers() {
    let mut save = leaf("save", "Save");
    save.shortcut = Some(PlatformMenuShortcut::new("s").modifiers(ShortcutModifiers::CONTROL));
    let menu_snapshot = snapshot(vec![save.clone()]);
    assert_eq!(
        matching_menu_shortcut(&menu_snapshot, "S", ShortcutModifiers::CONTROL),
        Some("save".into())
    );
    assert_eq!(
        matching_menu_shortcut(
            &menu_snapshot,
            "s",
            ShortcutModifiers::CONTROL.union(ShortcutModifiers::SHIFT)
        ),
        None
    );
    save.enabled = false;
    assert_eq!(
        matching_menu_shortcut(&snapshot(vec![save]), "s", ShortcutModifiers::CONTROL),
        None
    );
}

#[test]
fn native_structure_ignores_attribute_only_changes() {
    let first = snapshot(vec![PlatformMenuSnapshotNode {
        id: "file".into(),
        label: "File".to_owned(),
        tooltip: None,
        shortcut: None,
        enabled: true,
        selectable: false,
        separator: false,
        children: vec![leaf("open", "Open")],
    }]);
    let mut changed = first.clone();
    changed.menus[0].label = "Document".to_owned();
    changed.menus[0].children[0].enabled = false;
    assert!(native_menu_structure_equal(&first, &changed));
    changed.menus[0].children.push(leaf("save", "Save"));
    assert!(!native_menu_structure_equal(&first, &changed));
}
