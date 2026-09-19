use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::internal::{
    ActionInteractionController, ActionInteractionState, ActionPolicy, ActionSplashPolicy,
    ActionSurface, action_policy,
};
use incular_widgets::{Widget, internal::WidgetTree};
use std::time::{Duration, Instant};

#[test]
fn interaction_controller_notifies_only_on_effective_changes() {
    let controller = ActionInteractionController::new();
    let revision = controller.revision();
    assert_eq!(revision.get(), 0);

    let mut tree = WidgetTree::new();
    let button = Widget::from(
        ActionSurface::new("Action")
            .interaction_controller(controller.clone())
            .size(Size::new(80.0, 32.0)),
    );
    let id = tree.mount(button).expect("mount action");
    tree.layout(Constraints::loose(Size::new(120.0, 60.0)))
        .expect("layout");

    tree.set_button_interaction(id, Some(true), None, None)
        .expect("hover");
    assert_eq!(revision.get(), 1);
    assert_eq!(
        controller.state(),
        ActionInteractionState {
            hovered: true,
            pressed: false,
            focused: false,
        }
    );
    tree.set_button_interaction(id, Some(true), None, None)
        .expect("same hover");
    assert_eq!(revision.get(), 1, "no-op update must not notify");
}

#[test]
fn independently_mounted_actions_do_not_share_interaction_state() {
    let first = ActionInteractionController::new();
    let second = ActionInteractionController::new();
    let mut tree = WidgetTree::new();
    let root = incular_widgets::Column::new([
        Widget::from(
            ActionSurface::new("A")
                .interaction_controller(first.clone())
                .size(Size::new(80.0, 32.0)),
        ),
        Widget::from(
            ActionSurface::new("B")
                .interaction_controller(second.clone())
                .size(Size::new(80.0, 32.0)),
        ),
    ]);
    let root = tree.mount(root.into()).expect("mount actions");
    tree.layout(Constraints::loose(Size::new(120.0, 100.0)))
        .expect("layout");
    let first_id = tree.children(root).expect("children")[0];

    tree.set_button_interaction(first_id, Some(true), Some(true), None)
        .expect("interact first");
    assert!(first.state().hovered && first.state().pressed);
    assert_eq!(second.state(), ActionInteractionState::default());
}

#[test]
fn unmounted_presentation_has_no_subscriptions_or_animation_work() {
    let controller = ActionInteractionController::new();
    let revision = controller.revision();
    let mut tree = WidgetTree::new();
    let action = tree
        .mount(Widget::from(
            ActionSurface::new("Action")
                .interaction_controller(controller.clone())
                .size(Size::new(80.0, 32.0)),
        ))
        .expect("mount action");
    tree.layout(Constraints::loose(Size::new(120.0, 60.0)))
        .expect("layout");
    tree.set_button_interaction(action, Some(true), Some(true), Some(true))
        .expect("present interaction");
    assert_eq!(revision.get(), 1);

    tree.mount(incular_widgets::SizedBox::shrink().into())
        .expect("replace and unmount action");
    assert!(!tree.element_exists(action));
    let before = tree.diagnostics().animation_ticks;
    let origin = Instant::now();
    tree.update_compositor(origin)
        .expect("idle compositor update");
    tree.update_compositor(origin + Duration::from_secs(1))
        .expect("later idle compositor update");

    assert_eq!(tree.diagnostics().animation_ticks, before);
    assert_eq!(
        revision.get(),
        1,
        "detached action cannot publish new presentation"
    );
}

#[test]
fn action_policy_reaches_the_retained_action_owner() {
    let policy = ActionPolicy {
        feedback_enabled: false,
        splash: ActionSplashPolicy::Sparkle,
        transition_duration: Duration::from_millis(175),
    };
    let widget = Widget::from(ActionSurface::new("Action").policy(policy));
    assert_eq!(action_policy(&widget), Some(policy));
}
