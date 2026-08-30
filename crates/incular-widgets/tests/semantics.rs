//! Semantic widget behavior tests.

use incular_semantics::{SemanticAction, SemanticActionKind, SemanticRole};
use incular_widgets::{Semantics, SizedBox};

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
    assert_eq!(explicit.state.checked, Some(true));
    assert!(explicit.state.focused);
    assert!(explicit.state.read_only);
    assert!(explicit.state.multiline);
    assert_eq!(
        explicit.actions,
        vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
    );
}
