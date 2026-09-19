use incular_config::Constraints;
use incular_controls::{autocomplete, number_field, otp_field, select, tabs};
use incular_core::{Offset, Size};
use incular_semantics::Role;
use incular_widgets::{Text, Widget, internal::WidgetTree};
use std::{cell::RefCell, rc::Rc};

fn layout(tree: &mut WidgetTree) {
    tree.layout(Constraints::tight(Size::new(240.0, 80.0)))
        .expect("layout compound control");
}

#[test]
fn select_item_disabled_and_value_reach_selection_owner() {
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let output = changes.clone();
    let root = select::Root::new()
        .on_value_change(move |value| output.borrow_mut().push(value))
        .child(select::Item::new("grace", Text::new("Grace")));
    let mut tree = WidgetTree::new();
    tree.mount(root.into()).expect("mount select");
    layout(&mut tree);
    let handlers = tree.take_pending_handlers();
    assert_eq!(handlers.len(), 1);
    (handlers[0].1)();
    assert_eq!(changes.borrow().as_slice(), ["grace"]);
    layout(&mut tree);
    tree.update_semantics();
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| { node.role == Role::ListItem && node.state.selected })
    );

    let disabled = select::Root::new()
        .child(select::Item::new("disabled", Text::new("Disabled")).disabled(true));
    let mut tree = WidgetTree::new();
    tree.mount(disabled.into()).expect("mount disabled select");
    layout(&mut tree);
    assert!(tree.take_pending_handlers().is_empty());
}

#[test]
fn number_field_step_bounds_and_callback_reach_shared_model() {
    let values = Rc::new(RefCell::new(Vec::<f64>::new()));
    let output = values.clone();
    let root = number_field::Root::new()
        .value(1.0)
        .min(0.0)
        .max(3.0)
        .step(2.0)
        .on_value_change(move |value| output.borrow_mut().push(value))
        .child(number_field::Increment::new(Text::new("+")));
    let mut tree = WidgetTree::new();
    tree.mount(root.into()).expect("mount number field");
    layout(&mut tree);
    let handlers = tree.take_pending_handlers();
    assert_eq!(handlers.len(), 1);
    (handlers[0].1)();
    assert_eq!(values.borrow().as_slice(), [3.0]);
    (handlers[0].1)();
    assert_eq!(
        values.borrow().as_slice(),
        [3.0],
        "max prevents duplicate change"
    );
}

#[test]
fn autocomplete_semantic_set_text_reaches_editor() {
    let queries = Rc::new(RefCell::new(Vec::<String>::new()));
    let output = queries.clone();
    let root = autocomplete::Root::new()
        .query("Ada")
        .on_query_change(move |value| output.borrow_mut().push(value));
    let mut tree = WidgetTree::new();
    tree.mount(root.into()).expect("mount autocomplete");
    layout(&mut tree);
    let editor = tree
        .text_field_at(Offset::new(20.0, 20.0))
        .expect("autocomplete owns a real editor");
    let controller = tree.text_controller(editor).expect("editor controller");
    controller.set_text("Grace");
    assert_eq!(queries.borrow().as_slice(), ["Grace"]);
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(editor)
        .expect("editor semantics");
    assert_eq!(
        tree.semantics().node(semantic).expect("semantic node").role,
        Role::TextField
    );
}

#[test]
fn otp_uses_one_real_editor_for_default_and_custom_visuals() {
    for widget in [
        Widget::from(otp_field::Root::new(6).masked(true)),
        Widget::from(otp_field::Root::new(4).child(Text::new("custom slots"))),
    ] {
        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount otp");
        layout(&mut tree);
        assert!(
            tree.text_field_at(Offset::new(20.0, 20.0)).is_some(),
            "OTP must retain one editable owner"
        );
    }
}

#[test]
fn tabs_value_and_disabled_state_reach_shared_selection_owner() {
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let output = changes.clone();
    let root = tabs::Root::new()
        .on_value_change(move |value| output.borrow_mut().push(value))
        .child(tabs::Tab::new("settings", Text::new("Settings")));
    let mut tree = WidgetTree::new();
    tree.mount(root.into()).expect("mount tabs");
    layout(&mut tree);
    let handlers = tree.take_pending_handlers();
    assert_eq!(handlers.len(), 1);
    (handlers[0].1)();
    assert_eq!(changes.borrow().as_slice(), ["settings"]);
    layout(&mut tree);
    tree.update_semantics();
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| { node.role == Role::Tab && node.state.selected })
    );

    let disabled =
        tabs::Root::new().child(tabs::Tab::new("disabled", Text::new("Disabled")).disabled(true));
    let mut tree = WidgetTree::new();
    tree.mount(disabled.into()).expect("mount disabled tab");
    layout(&mut tree);
    assert!(tree.take_pending_handlers().is_empty());
}
