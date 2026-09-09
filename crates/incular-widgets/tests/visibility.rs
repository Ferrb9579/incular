//! Behavioral coverage for retained visibility, including transitions.

use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Offset, Size};
use incular_widgets::internal::{ActionId, ScaleController, WidgetTree, action};
use incular_widgets::{Offstage, OverlayPortal, Padding, ScaleTransition, Visibility, Widget};
use std::time::{Duration, Instant};

fn child() -> Widget {
    action(Size::new(20., 30.), Color::WHITE, ActionId(1))
        .accessibility_label("retained child")
        .with_key("child")
}

#[test]
fn every_preservation_combination_has_the_same_builder_and_fluent_behavior() {
    for flags in 0..16 {
        let state = flags & 1 != 0;
        let size = flags & 2 != 0;
        let animation = flags & 4 != 0;
        let semantics = flags & 8 != 0;
        for generated in [false, true] {
            let make = |visible| -> Widget {
                let visibility: Widget = if generated {
                    Visibility::builder()
                        .child(child())
                        .visible(visible)
                        .maintain_state(state)
                        .maintain_size(size)
                        .maintain_animation(animation)
                        .maintain_semantics(semantics)
                        .replacement(Widget::box_(Size::new(7., 9.), Color::BLACK))
                        .build()
                        .into()
                } else {
                    Visibility::new(child())
                        .visible(visible)
                        .maintain_state(state)
                        .maintain_size(size)
                        .maintain_animation(animation)
                        .maintain_semantics(semantics)
                        .replacement(Widget::box_(Size::new(7., 9.), Color::BLACK))
                        .into()
                };
                Padding::new(EdgeInsets::all(0.), visibility).into()
            };
            let mut tree = WidgetTree::new();
            let root = tree.mount(make(true)).unwrap();
            let wrapper = tree.children(root).unwrap()[0];
            let retained = tree.children(wrapper).unwrap()[0];
            let constraints = Constraints::loose(Size::new(100., 100.));
            tree.layout(constraints).unwrap();
            let visible_paint = tree.paint();
            tree.update(root, make(false)).unwrap();
            tree.layout(constraints).unwrap();
            tree.update_semantics();
            let expected = if flags == 0 {
                Size::new(7., 9.)
            } else if size {
                Size::new(20., 30.)
            } else {
                Size::ZERO
            };
            assert_eq!(
                tree.render_size(tree.render_id(root).unwrap()),
                Some(expected),
                "flags={flags}, generated={generated}"
            );
            assert_eq!(
                tree.semantics_debug_dump().contains("retained child"),
                semantics
            );
            if flags != 0 {
                assert_eq!(tree.children(wrapper).unwrap()[0], retained);
                assert_eq!(
                    tree.render_size(tree.render_id(retained).unwrap()),
                    Some(Size::new(20., 30.))
                );
                let hit = tree
                    .hit_test(Offset::new(5., 5.))
                    .and_then(|render| tree.element_for_render(render));
                assert_ne!(hit, Some(retained));
                assert_ne!(hit, Some(wrapper));
                assert_eq!(tree.focusable_elements(), vec![retained]);
                let hidden_paint = tree.paint();
                assert!(
                    hidden_paint.is_empty(),
                    "cached paint leaked for flags={flags}: {hidden_paint:?}"
                );
                tree.update(root, make(true)).unwrap();
                tree.layout(constraints).unwrap();
                assert_eq!(tree.children(wrapper).unwrap()[0], retained);
                assert_eq!(tree.paint(), visible_paint);
                assert!(tree.hit_test(Offset::new(5., 5.)).is_some());
            } else {
                assert!(tree.render_id(retained).is_none());
            }
        }
    }
}

#[test]
fn hidden_animation_ticks_are_opt_in_and_resume_at_elapsed_time() {
    for maintained in [false, true] {
        let controller = ScaleController::new();
        let make = |visible| -> Widget {
            Visibility::new(ScaleTransition::new(controller.clone(), child()))
                .visible(visible)
                .maintain_state(true)
                .maintain_animation(maintained)
                .into()
        };
        let mut tree = WidgetTree::new();
        let root = tree.mount(make(false)).unwrap();
        tree.layout(Constraints::loose(Size::new(100., 100.)))
            .unwrap();
        let start = Instant::now();
        controller.animate_to(3., Duration::from_secs(1), start);
        tree.update_compositor(start).unwrap();
        let (_, active) = tree
            .update_compositor(start + Duration::from_millis(500))
            .unwrap();
        assert_eq!(active, maintained);
        assert_eq!(controller.scale(), if maintained { 2. } else { 1. });
        tree.update(root, make(true)).unwrap();
        tree.update_compositor(start + Duration::from_secs(1))
            .unwrap();
        assert_eq!(controller.scale(), 3.);
    }
}

#[test]
fn outer_visibility_mutes_an_inner_animation_opt_in() {
    let controller = ScaleController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        Visibility::new(
            Visibility::new(ScaleTransition::new(controller.clone(), child()))
                .visible(false)
                .maintain_animation(true),
        )
        .visible(false)
        .maintain_state(true)
        .into(),
    )
    .unwrap();
    let start = Instant::now();
    controller.animate_to(3., Duration::from_secs(1), start);
    tree.update_compositor(start).unwrap();
    assert!(
        !tree
            .update_compositor(start + Duration::from_millis(500))
            .unwrap()
            .1
    );
    assert_eq!(controller.scale(), 1.);
}

#[test]
fn offstage_builder_matches_fluent_construction() {
    assert_eq!(
        Offstage::builder().child(child()).build(),
        Offstage::new(child())
    );
    assert_eq!(
        Offstage::builder().child(child()).offstage(false).build(),
        Offstage::new(child()).offstage(false)
    );
}

#[test]
fn offstage_measures_without_occupying_space_and_keeps_ticking() {
    let controller = ScaleController::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Offstage::new(ScaleTransition::new(controller.clone(), child())).into())
        .unwrap();
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .unwrap();
    let child = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::ZERO)
    );
    assert_eq!(
        tree.render_size(tree.render_id(child).unwrap()),
        Some(Size::new(20., 30.))
    );
    assert!(tree.paint().is_empty());
    tree.update_semantics();
    assert!(tree.semantics().is_empty());
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
    let start = Instant::now();
    controller.animate_to(3., Duration::from_secs(1), start);
    tree.update_compositor(start).unwrap();
    assert!(
        tree.update_compositor(start + Duration::from_millis(500))
            .unwrap()
            .1
    );
    assert_eq!(controller.scale(), 2.);
}

#[test]
fn changing_maintain_animation_while_hidden_takes_effect_without_layout_or_paint() {
    let controller = ScaleController::new();
    let make = |animation: bool| -> Widget {
        Visibility::new(ScaleTransition::new(controller.clone(), child()))
            .visible(false)
            .maintain_state(true)
            .maintain_animation(animation)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make(false)).unwrap();
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .unwrap();
    let retained = tree.children(root).unwrap()[0];
    let start = Instant::now();
    controller.animate_to(3., Duration::from_secs(1), start);
    tree.update_compositor(start).unwrap();
    assert!(
        !tree
            .update_compositor(start + Duration::from_millis(500))
            .unwrap()
            .1
    );
    assert_eq!(controller.scale(), 1.);
    let before = tree.diagnostics();
    tree.update(root, make(true)).unwrap();
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .unwrap();
    assert_eq!(tree.children(root).unwrap()[0], retained);
    assert!(
        tree.update_compositor(start + Duration::from_millis(500))
            .unwrap()
            .1
    );
    assert_eq!(controller.scale(), 2.);
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert!(tree.paint().is_empty());
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
}

#[test]
fn changing_maintain_semantics_while_hidden_toggles_semantics_without_paint() {
    let make = |semantics: bool| -> Widget {
        Visibility::new(child())
            .visible(false)
            .maintain_state(true)
            .maintain_semantics(semantics)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make(false)).unwrap();
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .unwrap();
    let retained = tree.children(root).unwrap()[0];
    tree.update_semantics();
    assert!(!tree.semantics_debug_dump().contains("retained child"));
    let before = tree.diagnostics();
    tree.update(root, make(true)).unwrap();
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .unwrap();
    assert_eq!(tree.children(root).unwrap()[0], retained);
    tree.update_semantics();
    assert!(tree.semantics_debug_dump().contains("retained child"));
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert!(tree.paint().is_empty());
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
    tree.update(root, make(false)).unwrap();
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .unwrap();
    assert_eq!(tree.children(root).unwrap()[0], retained);
    tree.update_semantics();
    assert!(!tree.semantics_debug_dump().contains("retained child"));
}

#[test]
fn changing_maintain_size_while_hidden_resizes_through_layout_only() {
    let make = |size: bool| -> Widget {
        Visibility::new(child())
            .visible(false)
            .maintain_state(true)
            .maintain_size(size)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make(false)).unwrap();
    let constraints = Constraints::loose(Size::new(100., 100.));
    tree.layout(constraints).unwrap();
    let retained = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::ZERO)
    );
    let before = tree.diagnostics();
    tree.update(root, make(true)).unwrap();
    tree.layout(constraints).unwrap();
    assert_eq!(tree.children(root).unwrap()[0], retained);
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(20., 30.))
    );
    assert_eq!(
        tree.render_size(tree.render_id(retained).unwrap()),
        Some(Size::new(20., 30.))
    );
    let after = tree.diagnostics();
    assert!(after.layouts > before.layouts);
    assert_eq!(after.paints, before.paints);
    assert!(tree.paint().is_empty());
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
    tree.update(root, make(false)).unwrap();
    tree.layout(constraints).unwrap();
    assert_eq!(tree.children(root).unwrap()[0], retained);
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::ZERO)
    );
}

#[test]
fn reapplying_identical_hidden_configuration_schedules_no_phases() {
    let make = || -> Widget {
        Visibility::new(child())
            .visible(false)
            .maintain_state(true)
            .maintain_size(true)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make()).unwrap();
    let constraints = Constraints::loose(Size::new(100., 100.));
    tree.layout(constraints).unwrap();
    let retained = tree.children(root).unwrap()[0];
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(root, make()).unwrap();
    tree.layout(constraints).unwrap();
    let _ = tree.paint();
    assert_eq!(tree.children(root).unwrap()[0], retained);
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
}

#[test]
fn hidden_ancestors_suppress_nested_popup_surfaces_and_pointer_hits() {
    let make = |visible| -> Widget {
        let nested = OverlayPortal::new(child())
            .overlay_child(child())
            .show(true);
        Visibility::new(OverlayPortal::new(child()).overlay_child(nested).show(true))
            .visible(visible)
            .maintain_size(true)
            .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make(true)).unwrap();
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).unwrap();
    assert_eq!(tree.transient_surfaces().len(), 2);
    let _ = tree.paint();
    tree.update(root, make(false)).unwrap();
    tree.layout(constraints).unwrap();
    assert!(tree.transient_surfaces().is_empty());
    assert!(tree.hit_test(Offset::new(5., 35.)).is_none());
    assert!(tree.paint().is_empty());
    tree.update(root, make(true)).unwrap();
    tree.layout(constraints).unwrap();
    assert_eq!(tree.transient_surfaces().len(), 2);
}
