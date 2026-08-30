use incular_core::{ArenaId, Offset, Rect, Size};
use incular_semantics::{Role, SemanticNode, SemanticNodeId, SemanticState, SemanticsTree};

fn node(id: SemanticNodeId) -> SemanticNode {
    SemanticNode {
        id,
        role: Role::Text,
        label: None,
        value: None,
        description: None,
        bounds: Rect::from_origin_size(Offset::ZERO, Size::ZERO),
        state: SemanticState::default(),
        actions: vec![],
        children: vec![],
    }
}

#[test]
fn stale_ids_do_not_address_reused_nodes() {
    let mut tree = SemanticsTree::new();
    let first = tree.insert(node(SemanticNodeId(ArenaId::from_parts(0, 0))));
    tree.remove(first);
    let replacement = tree.insert(node(first));
    assert_ne!(first, replacement);
    assert!(tree.node(first).is_none());
    assert!(tree.node(replacement).is_some());
}

#[test]
fn updates_distinguish_property_and_geometry_changes() {
    let mut tree = SemanticsTree::new();
    let id = tree.insert(node(SemanticNodeId(ArenaId::from_parts(0, 0))));
    let mut updated = tree.node(id).expect("inserted node").clone();
    updated.label = Some("Heading".into());
    tree.update(id, updated.clone());
    updated.bounds = Rect::from_origin_size(Offset::new(4.0, 8.0), Size::new(10.0, 20.0));
    tree.update(id, updated);
    assert_eq!(tree.diagnostics().nodes_updated, 2);
    assert_eq!(tree.diagnostics().property_updates, 1);
    assert_eq!(tree.diagnostics().geometry_updates, 1);
}
