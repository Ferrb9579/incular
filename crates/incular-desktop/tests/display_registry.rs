#[path = "../src/display_identity.rs"]
mod display_identity;

use display_identity::GenerationalDisplayRegistry;

#[test]
fn removed_display_slot_advances_generation_before_reuse() {
    let mut registry = GenerationalDisplayRegistry::default();
    registry.synchronize(&["left", "right"]);
    let left = registry.id_for(&"left").expect("left display");
    let right = registry.id_for(&"right").expect("right display");

    registry.synchronize(&["right"]);
    assert!(registry.id_for(&"left").is_none());
    assert_eq!(registry.id_for(&"right"), Some(right));

    registry.synchronize(&["right", "replacement"]);
    let replacement = registry.id_for(&"replacement").expect("replacement");
    assert_eq!(replacement.index(), left.index());
    assert_eq!(replacement.generation(), left.generation() + 1);
    assert_ne!(replacement, left);
}

#[test]
fn enumeration_order_does_not_change_display_identity() {
    let mut registry = GenerationalDisplayRegistry::default();
    registry.synchronize(&[1_u8, 2_u8, 3_u8]);
    let ids = [1_u8, 2, 3].map(|key| registry.id_for(&key).expect("display"));
    assert_eq!(registry.connected().count(), 3);
    registry.synchronize(&[3_u8, 1_u8, 2_u8]);
    assert_eq!(
        [1_u8, 2, 3].map(|key| registry.id_for(&key).expect("display")),
        ids
    );
}
