use super::*;

#[test]
fn hit_test_resolves_button_action() {
    let mut runtime = Runtime::new(incular_widgets::internal::action(
        Size::new(10., 10.),
        Color::WHITE,
        ActionId(1),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    assert_eq!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Up,
                position: Offset::new(5., 5.)
            })
            .unwrap()
            .action,
        Some(ActionId(1))
    );
}

#[test]
fn identified_primary_contact_can_activate_a_button() {
    let mut runtime = Runtime::new(incular_widgets::internal::action(
        Size::new(10., 10.),
        Color::WHITE,
        ActionId(2),
    ))
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(20., 20.)))
        .unwrap();
    let _ = runtime.handle_input(InputEvent::PointerWithId {
        pointer: 72,
        phase: PointerPhase::Down,
        position: Offset::new(5., 5.),
    });
    assert_eq!(
        runtime
            .handle_input(InputEvent::PointerWithId {
                pointer: 72,
                phase: PointerPhase::Up,
                position: Offset::new(5., 5.),
            })
            .unwrap()
            .action,
        Some(ActionId(2))
    );
}

#[test]
fn button_hover_callbacks_fire_once_on_enter_and_exit() {
    let enters = Rc::new(Cell::new(0_u32));
    let exits = Rc::new(Cell::new(0_u32));
    let mut runtime = Runtime::new(
        ActionSurface::new("Hover")
            .on_hover({
                let enters = enters.clone();
                move || enters.set(enters.get() + 1)
            })
            .on_exit({
                let exits = exits.clone();
                move || exits.set(exits.get() + 1)
            })
            .into(),
    )
    .unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(100., 40.)))
        .unwrap();
    for position in [
        Offset::new(10., 10.),
        Offset::new(20., 10.),
        Offset::new(150., 10.),
        Offset::new(160., 10.),
    ] {
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Move,
            position,
        });
    }
    assert_eq!(enters.get(), 1);
    assert_eq!(exits.get(), 1);
}

#[test]
fn wheel_updates_only_retained_scroll_transform() {
    let controller = incular_widgets::ScrollController::new();
    let child = Widget::column(
        (0..8)
            .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
            .collect::<Vec<_>>(),
    );
    let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), child)).unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let (_, initial) = runtime.run_frame(constraints).unwrap();
    assert!(
        initial.laid_out_render_objects > 0
            && initial.repainted_render_objects > 0
            && initial.composited > 0
    );
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Move,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Scroll {
        delta: Offset::new(0., 60.),
    });
    let (_, frame) = runtime.run_frame(constraints).unwrap();
    assert_eq!(frame.rebuilt_elements, 0);
    assert_eq!(frame.laid_out_render_objects, 0);
    assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
    assert!(frame.composited > 0);
    assert_eq!(controller.offset(), 60.);
}

#[test]
fn translated_button_hit_tests_at_its_visible_position_without_repaint() {
    let controller = incular_widgets::internal::TranslationController::new();
    controller.set_offset(Offset::new(0., 30.));
    let mut runtime = Runtime::new(Widget::translate(
        controller.clone(),
        incular_widgets::internal::action(Size::new(20., 20.), Color::WHITE, ActionId(9)),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let _ = runtime.run_frame(constraints).unwrap();
    let (_, frame) = runtime.run_frame(constraints).unwrap();
    assert_eq!(frame.repainted_render_objects, 0);
    assert!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 35.)
            })
            .is_some()
    );
    assert!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 5.)
            })
            .is_none()
    );
}

#[test]
fn scrolled_button_hits_at_visible_not_old_location() {
    let controller = incular_widgets::ScrollController::new();
    let content = Widget::column(vec![
        Widget::box_(Size::new(80., 160.), Color::WHITE),
        incular_widgets::internal::action(Size::new(20., 20.), Color::WHITE, ActionId(12)),
    ]);
    let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), content)).unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let _ = runtime.run_frame(constraints).unwrap();
    assert!(controller.jump_to(80.));
    let (_, frame) = runtime.run_frame(constraints).unwrap();
    assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
    assert_eq!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 85.)
            })
            .unwrap()
            .action,
        Some(ActionId(12))
    );
    assert!(
        runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(5., 165.)
            })
            .is_none()
    );
}

#[test]
fn animation_ticks_request_frames_without_rebuild_or_paint() {
    let controller = incular_widgets::internal::TranslationController::new();
    let mut runtime = Runtime::new(Widget::translate(
        controller.clone(),
        Widget::text("warm text"),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 100.));
    let origin = Instant::now();
    let _ = runtime.run_frame_at(constraints, origin).unwrap();
    let text_before = runtime.tree().text_diagnostics();
    controller.animate_to(Offset::new(50., 0.), Duration::from_millis(1000), origin);
    let (_, frame) = runtime
        .run_frame_at(constraints, origin + Duration::from_millis(500))
        .unwrap();
    assert_eq!(frame.rebuilt_elements, 0);
    assert_eq!(frame.laid_out_render_objects, 0);
    assert_eq!(frame.repainted_render_objects, 0);
    assert!(frame.composited > 0 && frame.active_animations > 0 && frame.requested_another_frame);
    assert_eq!(runtime.tree().text_diagnostics(), text_before);
}

#[test]
fn retained_card_text_and_background_move_together_without_repaint() {
    let controller = incular_widgets::internal::TranslationController::new();
    let mut runtime = Runtime::new(Widget::padding(
        incular_config::EdgeInsets {
            left: 20.,
            top: 10.,
            right: 0.,
            bottom: 0.,
        },
        Widget::translate(
            controller.clone(),
            Widget::column(vec![
                Widget::box_(Size::new(80., 20.), Color::WHITE),
                Widget::text("cached card text"),
            ]),
        ),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(200., 100.));
    let (before, _) = runtime.run_frame(constraints).unwrap();
    let text_before = runtime.tree().text_diagnostics();
    let (before_rect, before_glyph) = picture_origins(&before);
    controller.set_offset(Offset::new(50., 0.));
    let (after, frame) = runtime.run_frame(constraints).unwrap();
    let (after_rect, after_glyph) = picture_origins(&after);
    assert_eq!(frame.rebuilt_elements, 0);
    assert_eq!(frame.laid_out_render_objects, 0);
    assert_eq!(frame.repainted_render_objects, 0);
    assert!(frame.composited > 0);
    assert_eq!(after_rect - before_rect, Offset::new(50., 0.));
    assert_eq!(after_glyph - before_glyph, Offset::new(50., 0.));
    assert_eq!(runtime.tree().text_diagnostics(), text_before);
}

#[test]
fn declarative_button_dispatches_only_a_completed_press() {
    let hits = Rc::new(Cell::new(0));
    let callback_hits = hits.clone();
    let app = Application::new(move |_| {
        ActionSurface::new("Add")
            .on_press({
                let callback_hits = callback_hits.clone();
                move || callback_hits.set(callback_hits.get() + 1)
            })
            .into()
    })
    .unwrap();
    let mut runtime = app.into_runtime();
    runtime
        .run_frame(Constraints::tight(Size::new(120., 60.)))
        .unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(121., 50.),
    });
    assert_eq!(hits.get(), 0);
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(10., 10.),
    });
    assert_eq!(hits.get(), 1);
}

#[test]
fn decorated_ancestor_binds_nested_button_callback() {
    let hits = Rc::new(Cell::new(0));
    let observed = hits.clone();
    let root: Widget = DecoratedBox::new(
        ActionSurface::new("Nested").on_press(move || observed.set(observed.get() + 1)),
    )
    .background(Color::BLACK)
    .into();
    let mut runtime = Runtime::new(root).unwrap();
    runtime
        .run_frame(Constraints::tight(Size::new(120., 60.)))
        .unwrap();
    let down = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Down,
        position: Offset::new(10., 10.),
    });
    let up = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(10., 10.),
    });
    assert!(down.is_some_and(|target| target.action.is_some()));
    assert!(up.is_some_and(|target| target.action.is_some()));
    assert_eq!(hits.get(), 1);
}

#[test]
fn sliver_list_keeps_small_scrolls_compositor_only_and_direct_jumps_bounded() {
    let controller = incular_widgets::ScrollController::new();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut runtime = Runtime::new(fixed_sliver_list(
        1_000_000,
        40.,
        controller.clone(),
        move |index| {
            observed.set(observed.get() + 1);
            Widget::text(format!("Item {index}"))
        },
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(120., 100.));
    let (_, initial) = runtime.run_frame(constraints).unwrap();
    assert!(initial.laid_out_render_objects > 0 && initial.repainted_render_objects > 0);
    let warm_calls = calls.get();
    let warm_text = runtime.tree().text_diagnostics();
    assert!(controller.jump_to(3.));
    let (_, small) = runtime.run_frame(constraints).unwrap();
    assert_eq!(calls.get(), warm_calls);
    assert_eq!(small.rebuilt_elements, 0);
    assert_eq!(small.laid_out_render_objects, 0);
    assert!(small.repainted_render_objects <= 1); // overlay scrollbar only
    assert!(small.composited > 0);
    assert_eq!(runtime.tree().text_diagnostics(), warm_text);

    let boundary_before = runtime.diagnostics();
    assert!(controller.jump_to(40.));
    let (_, boundary) = runtime.run_frame(constraints).unwrap();
    assert_eq!(calls.get(), warm_calls + 1);
    let boundary_after = runtime.diagnostics();
    assert_eq!(boundary_after.items_built - boundary_before.items_built, 1);
    assert_eq!(
        boundary_after.items_mounted - boundary_before.items_mounted,
        1
    );
    assert_eq!(
        boundary_after.items_unmounted - boundary_before.items_unmounted,
        0
    );
    assert!(boundary.laid_out_render_objects <= 2);
    assert!(boundary.repainted_render_objects <= 2);

    assert!(controller.jump_to(900_000. * 40.));
    let (_, jumped) = runtime.run_frame(constraints).unwrap();
    let diagnostics = runtime.tree().sliver_viewport_diagnostics().unwrap();
    assert!(diagnostics.materialized_range.contains(&900_000));
    assert!(diagnostics.materialized_item_count < 100);
    assert!(calls.get() < 200);
    assert!(jumped.laid_out_render_objects < 100);
    assert!(diagnostics.picture_layer_count < 250);
}

#[test]
fn sliver_button_rows_hit_their_logical_index_and_drop_stale_handlers() {
    let controller = incular_widgets::ScrollController::new();
    let hit = Rc::new(Cell::new(None));
    let observed = hit.clone();
    let mut runtime = Runtime::new(fixed_sliver_list(
        2_000,
        40.,
        controller.clone(),
        move |index| {
            let hit = observed.clone();
            ActionSurface::new(format!("Item {index}")).on_press(move || hit.set(Some(index)))
        },
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(120., 40.));
    runtime.run_frame(constraints).unwrap();
    assert!(controller.jump_to(500. * 40.));
    runtime.run_frame(constraints).unwrap();
    let down = runtime
        .handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        })
        .unwrap();
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase: PointerPhase::Up,
        position: Offset::new(10., 10.),
    });
    assert_eq!(hit.get(), Some(500));
    let stale_action = down.action.unwrap();
    assert!(controller.jump_to(1_500. * 40.));
    runtime.run_frame(constraints).unwrap();
    assert!(!runtime.handlers.contains_key(&stale_action));
}

#[test]
fn long_sliver_scroll_keeps_retained_resources_bounded() {
    let controller = incular_widgets::ScrollController::new();
    let mut runtime = Runtime::new(fixed_sliver_list(
        50_000,
        40.,
        controller.clone(),
        move |index| ActionSurface::new(format!("Item {index}")),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(120., 120.));
    runtime.run_frame(constraints).unwrap();
    for index in (97..10_000).step_by(97) {
        assert!(controller.jump_to(index as f32 * 40.));
        runtime.run_frame(constraints).unwrap();
        let view = runtime.tree().sliver_viewport_diagnostics().unwrap();
        assert!(view.materialized_item_count < 30);
        assert!(view.element_count < 100);
        assert!(view.render_object_count < 100);
        assert!(view.picture_layer_count < 200);
        assert!(runtime.handlers.len() < 30);
    }
}
