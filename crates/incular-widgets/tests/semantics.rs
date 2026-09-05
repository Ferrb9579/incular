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
