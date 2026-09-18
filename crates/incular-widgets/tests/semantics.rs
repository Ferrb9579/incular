//! Semantic widget behavior tests.

use incular_config::Constraints;
use incular_core::Size;
use incular_semantics::{SemanticAction, SemanticActionKind, SemanticRole};
use incular_widgets::{
    Column, Semantics, SizedBox, Text, Widget,
    internal::{ExplicitSemantics, WidgetTree},
};

#[test]
fn semantic_state_and_explicit_actions_survive_widget_conversion() {
    let semantics = Semantics::new(SizedBox::shrink())
        .role(SemanticRole::Checkbox)
        .label("Remember me")
        .value("on")
        .enabled(true)
        .selected(true)
        .checked(true)
        .focused(true)
        .read_only(true)
        .multiline(true)
        .action(SemanticAction::Focus)
        .action(SemanticAction::Activate);

    let explicit = semantics
        .explicit()
        .expect("role creates explicit semantics");
    assert_eq!(explicit.role, SemanticRole::Checkbox);
    assert_eq!(explicit.label.as_deref(), Some("Remember me"));
    assert_eq!(explicit.value.as_deref(), Some("on"));
    assert!(explicit.state.enabled);
    assert!(explicit.state.selected);
    assert_eq!(explicit.state.checked, Some(true.into()));
    assert!(explicit.state.focused);
    assert!(explicit.state.read_only);
    assert!(explicit.state.multiline);
    assert_eq!(
        explicit.actions,
        vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
    );
}

#[test]
fn first_semantics_pass_wires_parent_child_edges() {
    let child: Widget = Text::new("child").into();
    let root: Widget = Widget::from(Column::new([child]))
        .semantics(ExplicitSemantics::new(SemanticRole::GenericContainer).label("parent"));
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("mount semantics tree");
    tree.layout(Constraints::tight(Size::new(160.0, 80.0)))
        .expect("layout semantics tree");

    tree.update_semantics();

    let root = tree.semantics().root().expect("semantic root");
    let root = tree.semantics().node(root).expect("semantic root node");
    assert_eq!(root.label.as_deref(), Some("parent"));
    assert_eq!(root.children.len(), 1);
    let child = tree
        .semantics()
        .node(root.children[0])
        .expect("semantic child node");
    assert_eq!(child.label.as_deref(), Some("child"));
}

#[test]
fn semantic_node_ids_survive_reorder_and_prune_removal() {
    let labeled = |key: u64, label: &str| {
        Widget::from(Text::new(label))
            .with_key(key)
            .semantics(ExplicitSemantics::new(SemanticRole::Group).label(label.to_owned()))
    };
    let key_for = |label: &str| match label {
        "a" => 1,
        "b" => 2,
        "c" => 3,
        _ => 0,
    };
    let build = |labels: &[&str]| {
        Widget::from(Column::new(
            labels
                .iter()
                .map(|label| labeled(key_for(label), label))
                .collect::<Vec<_>>(),
        ))
        .semantics(ExplicitSemantics::new(SemanticRole::GenericContainer).label("parent"))
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(build(&["a", "b"])).expect("mount");
    tree.layout(Constraints::tight(Size::new(160., 80.)))
        .unwrap();
    tree.update_semantics();
    let node_for = |tree: &WidgetTree, label: &str| {
        tree.semantics()
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some(label))
            .map(|(id, _)| id)
    };
    let a_before = node_for(&tree, "a").expect("a node");
    let b_before = node_for(&tree, "b").expect("b node");
    tree.update(root, build(&["b", "a", "c"])).unwrap();
    tree.layout(Constraints::tight(Size::new(160., 80.)))
        .unwrap();
    tree.update_semantics();
    assert_eq!(node_for(&tree, "a"), Some(a_before));
    assert_eq!(node_for(&tree, "b"), Some(b_before));
    assert!(node_for(&tree, "c").is_some());
    tree.update(root, build(&["a"])).unwrap();
    tree.layout(Constraints::tight(Size::new(160., 80.)))
        .unwrap();
    tree.update_semantics();
    assert_eq!(node_for(&tree, "a"), Some(a_before));
    assert!(node_for(&tree, "b").is_none());
}
