use incular_config::Constraints;
use incular_controls::{
    ControlTheme,
    checkbox::{CheckedState, Root},
};
use incular_core::{Color, Size};
use incular_semantics::{Role, SemanticState};
use incular_widgets::{Widget, internal::WidgetTree};

fn semantic_state(
    state: CheckedState,
    enabled: bool,
    read_only: bool,
    custom: bool,
) -> SemanticState {
    let mut checkbox = Root::new()
        .state(state)
        .enabled(enabled)
        .read_only(read_only)
        .required(true);
    if custom {
        checkbox = checkbox.child(Widget::box_(Size::new(24., 24.), Color::WHITE));
    }
    let mut tree = WidgetTree::new();
    tree.mount(checkbox.build(&ControlTheme::light())).unwrap();
    tree.layout(Constraints::loose(Size::new(200., 100.)))
        .unwrap();
    tree.update_semantics();
    let nodes = tree
        .semantics()
        .iter()
        .filter(|(_, node)| node.role == Role::Checkbox)
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 1);
    nodes[0].1.state.clone()
}

#[test]
fn custom_checkbox_visual_does_not_change_semantic_state() {
    for state in [
        CheckedState::Unchecked,
        CheckedState::Checked,
        CheckedState::Indeterminate,
    ] {
        for enabled in [false, true] {
            for read_only in [false, true] {
                let expected = semantic_state(state, enabled, read_only, false);
                assert_eq!(expected.checked, Some(state));
                assert_eq!(expected.enabled, enabled);
                assert_eq!(expected.focusable, enabled);
                assert_eq!(expected.read_only, read_only);
                assert!(expected.required);
                assert_eq!(
                    semantic_state(state, enabled, read_only, false),
                    semantic_state(state, enabled, read_only, true),
                    "state={state:?}, enabled={enabled}, read_only={read_only}"
                );
            }
        }
    }
}
