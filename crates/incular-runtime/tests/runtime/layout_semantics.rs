use super::*;

#[test]
fn rounded_text_inserted_by_a_layout_builder_is_frame_stable() {
    let open = Rc::new(Cell::new(false));
    let revision = Rc::new(Cell::new(0));
    let open_for_builder = open.clone();
    let root = Widget::stateful_layout_builder(revision.clone(), move |_, _| {
        if open_for_builder.get() {
            Container::new()
                .height(32.0)
                .padding(EdgeInsets::symmetric(12.0, 0.0))
                .decoration(BoxDecoration::new().border_radius(BorderRadius::circular(4.0)))
                .child(Align::new(
                    incular_config::Alignment::CENTER,
                    Text::new("New document"),
                ))
                .into()
        } else {
            Text::new("Menu").into()
        }
    });
    let root = DefaultTextStyle::new(TextStyle::default().font_size(14.0), root).into();
    let mut runtime = Runtime::new(root).expect("mount rounded text test");
    let constraints = Constraints::tight(Size::new(320.0, 120.0));
    runtime.run_frame(constraints).expect("initial frame");
    open.set(true);
    revision.set(1);
    runtime.run_frame(constraints).expect("opened frame");
    runtime.run_frame(constraints).expect("stable opened frame");
}

#[test]
fn semantic_actions_share_logical_button_and_editing_state() {
    use incular_widgets::{
        EditableText,
        internal::{ActionSurface, TextEditingController},
    };
    let hits = Rc::new(Cell::new(0));
    let controller = TextEditingController::with_text("Ada");
    let mut runtime = Runtime::new(Widget::from(incular_widgets::Column::new(vec![
        Widget::from(ActionSurface::new("Increment").on_press({
            let hits = hits.clone();
            move || hits.set(hits.get() + 1)
        })),
        Widget::from(EditableText::new(controller.clone())),
    ])))
    .unwrap();
    let root = runtime.tree().root().unwrap();
    runtime
        .schedule_update(
            root,
            Widget::from(incular_widgets::Column::new(vec![
                Widget::from(ActionSurface::new("Increment").on_press({
                    let hits = hits.clone();
                    move || hits.set(hits.get() + 1)
                })),
                Widget::from(EditableText::new(controller.clone())),
            ])),
        )
        .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(300., 200.)))
        .unwrap();
    let button = semantic_node(&runtime, SemanticRole::Button);
    let field = semantic_node(&runtime, SemanticRole::TextField);
    assert!(runtime.dispatch_semantic_action(button, SemanticAction::Activate));
    assert_eq!(hits.get(), 1);
    assert!(runtime.dispatch_semantic_action(field, SemanticAction::Focus));
    assert_eq!(
        runtime.focused_element(),
        runtime.tree().element_for_semantic_node(field)
    );
    assert!(runtime.dispatch_semantic_action(field, SemanticAction::SetText("hello".into())));
    assert!(
        runtime
            .dispatch_semantic_action(field, SemanticAction::SetSelection { base: 1, extent: 4 })
    );
    assert_eq!(controller.text(), "hello");
    assert_eq!(
        controller.value().selection,
        TextSelection { base: 1, extent: 4 }
    );
}

#[test]
fn semantic_callbacks_make_custom_controls_actionable() {
    let activations = Rc::new(Cell::new(0));
    let observed = activations.clone();
    let mut runtime = Runtime::new(
        incular_widgets::Semantics::new(Text::new("custom"))
            .on_tap(move || observed.set(observed.get() + 1))
            .into(),
    )
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(200., 50.)))
        .unwrap();
    let node = semantic_node(&runtime, SemanticRole::Button);
    assert!(runtime.dispatch_semantic_action(node, SemanticAction::Activate));
    assert_eq!(activations.get(), 1);
}

#[test]
fn sliver_list_semantics_are_bounded_and_follow_materialization() {
    let controller = incular_widgets::ScrollController::new();
    let mut runtime = Runtime::new(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        |index| ActionSurface::new(format!("Item {index}")),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(200., 600.)))
        .unwrap();
    let initial = runtime.tree().semantics().len();
    assert!(initial < 100, "{initial}");
    let _viewport = semantic_node(&runtime, SemanticRole::ScrollView);
    assert_eq!(
        runtime
            .tree()
            .sliver_viewport_diagnostics()
            .unwrap()
            .logical_item_count,
        1_000_000
    );
    controller.jump_to(900_000. * 40.);
    runtime
        .run_frame(Constraints::tight(Size::new(200., 600.)))
        .unwrap();
    assert!(runtime.tree().semantics().len() < 100);
    assert!(runtime.tree().semantics().iter().any(|(_, node)| {
        node.label
            .as_deref()
            .is_some_and(|label| label.contains("900000"))
    }));
}

#[test]
fn semantic_scroll_uses_existing_controller() {
    let controller = incular_widgets::ScrollController::new();
    let mut runtime = Runtime::new(incular_widgets::internal::ScrollView::vertical(
        controller.clone(),
        Widget::box_(Size::new(100., 2000.), Color::WHITE),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(100., 200.)))
        .unwrap();
    let scroll = semantic_node(&runtime, SemanticRole::ScrollView);
    assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollForward));
    assert!(controller.offset() > 0.);
    assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollBackward));
    assert_eq!(controller.offset(), 0.);
}
