//! Layout and retained painting tests.

mod common;

use common::*;
use incular_config::{Alignment, Axis, Constraints, EdgeInsets};
use incular_core::{Color, Offset, Rect, Size, Transform as CoreTransform};
use incular_rendering::{DisplayList, PaintCommand};
use incular_semantics::SemanticRole;
use incular_text::{RichText, TextAlign, TextOverflow, TextStyle};
use incular_widgets::internal::*;
use incular_widgets::*;
use std::{
    any::Any,
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

#[test]
fn nested_environment_scopes_keep_all_typed_values() {
    let outer = Color::rgba(12, 34, 56, 255);
    let inner = TextStyle::new().font_size(24.0);
    let environment = compose_environment(
        Some(Rc::new(inner.clone()) as Rc<dyn Any>),
        Some(Rc::new(outer) as Rc<dyn Any>),
    )
    .expect("nested environment");

    with_build_environment(Some(environment), || {
        assert_eq!(
            current_build_environment::<TextStyle>(),
            Some(inner.clone())
        );
        assert_eq!(current_build_environment::<Color>(), Some(outer));
    });
}

#[test]
fn default_text_style_is_resolved_for_descendant_text() {
    let default_style = TextStyle::new().font_size(24.0).color(Color::BLACK);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(DefaultTextStyle::new(default_style.clone(), Text::new("hello")).into())
        .expect("mount default text style");
    tree.layout(Constraints::tight(Size::new(200.0, 80.0)));

    let child = tree.children(root).expect("materialized text")[0];
    let RenderKind::Text { style, .. } = tree
        .render_object_kind(tree.render_id(child).expect("text render"))
        .expect("text render object")
    else {
        panic!("expected text render kind");
    };
    assert_eq!(style.size, default_style.size);
    assert_eq!(style.color, Color::BLACK);
}

#[test]
fn limited_and_overflow_boxes_apply_their_distinct_constraint_policies() {
    let mut limited = WidgetTree::new();
    let root = limited
        .mount(Widget::limited_box(
            20.,
            30.,
            Widget::fixed_box(Size::new(80., 80.), Color::WHITE),
        ))
        .unwrap();
    limited.layout(Constraints::new(0., f32::INFINITY, 0., f32::INFINITY));
    assert_eq!(
        limited.render_size(limited.render_id(root).unwrap()),
        Some(Size::new(20., 30.))
    );
    limited.layout(Constraints::new(0., 100., 0., 100.));
    assert_eq!(
        limited.render_size(limited.render_id(root).unwrap()),
        Some(Size::new(80., 80.))
    );

    let mut overflow = WidgetTree::new();
    let root = overflow
        .mount(Widget::overflow_box(
            None,
            Some(80.),
            None,
            Some(80.),
            Widget::fixed_box(Size::new(80., 80.), Color::WHITE),
        ))
        .unwrap();
    overflow.layout(Constraints::tight(Size::new(20., 20.)));
    let child = overflow.children(root).unwrap()[0];
    assert_eq!(
        overflow.render_size(overflow.render_id(root).unwrap()),
        Some(Size::new(20., 20.))
    );
    assert_eq!(
        overflow.render_size(overflow.render_id(child).unwrap()),
        Some(Size::new(80., 80.))
    );
}

#[test]
fn flexible_expanded_and_spacer_allocate_bounded_main_axis_space() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::row(vec![
            Widget::fixed_box(Size::new(10., 10.), Color::WHITE),
            Expanded::new(Widget::fixed_box(Size::new(1., 10.), Color::WHITE))
                .flex(2)
                .into(),
            Flexible::new(Widget::fixed_box(Size::new(15., 10.), Color::WHITE))
                .flex(1)
                .into(),
            Spacer::new().flex(1).into(),
        ]))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 20.)));
    let children = tree.children(root).unwrap().to_vec();
    assert_eq!(
        tree.render_size(tree.render_id(children[1]).unwrap()),
        Some(Size::new(45., 10.))
    );
    assert_eq!(
        tree.render_size(tree.render_id(children[2]).unwrap()),
        Some(Size::new(15., 10.))
    );
    assert_eq!(
        tree.render_size(tree.render_id(children[3]).unwrap()),
        Some(Size::new(22.5, 0.))
    );
}

#[test]
fn positioned_and_indexed_stacks_keep_only_the_selected_branch_interactive_and_semantic() {
    let positioned = Positioned::new(Widget::fixed_box(Size::new(100., 100.), Color::WHITE))
        .left(10.)
        .right(20.)
        .top(5.)
        .bottom(15.);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::stack(Alignment::TOP_LEFT, vec![positioned.into()]))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let positioned = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_size(tree.render_id(positioned).unwrap()),
        Some(Size::new(70., 80.))
    );
    assert_eq!(
        tree.render_origin(tree.render_id(positioned).unwrap()),
        Offset::new(10., 5.)
    );

    let mut indexed = WidgetTree::new();
    let _root = indexed
        .mount(
            IndexedStack::new([Widget::text("hidden"), Widget::text("shown")])
                .index(1)
                .into(),
        )
        .unwrap();
    indexed.layout(Constraints::tight(Size::new(100., 40.)));
    indexed.update_semantics();
    assert_eq!(indexed.semantics().len(), 1);
    assert!(indexed.semantics_debug_dump().contains("shown"));
    assert!(!indexed.semantics_debug_dump().contains("hidden"));
}

#[test]
fn layout_builder_rebuilds_only_when_constraints_change() {
    let builds = Rc::new(Cell::new(0));
    let observed = builds.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::layout_builder(move |constraints| {
            observed.set(observed.get() + 1);
            Widget::fixed_box(Size::new(constraints.max_width, 10.), Color::WHITE)
        }))
        .unwrap();
    tree.layout(Constraints::new(0., 30., 0., 20.));
    let child = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_size(tree.render_id(child).unwrap()),
        Some(Size::new(30., 10.))
    );
    tree.layout(Constraints::new(0., 30., 0., 20.));
    assert_eq!(builds.get(), 1);
    tree.layout(Constraints::new(0., 40., 0., 20.));
    assert_eq!(builds.get(), 2);
}

#[test]
fn layout_builder_rebuilds_when_its_descriptor_changes_at_same_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::layout_builder(|_| Widget::text("old child")))
        .unwrap();
    let constraints = Constraints::tight(Size::new(200., 40.));

    tree.layout(constraints);
    tree.update_semantics();
    assert!(tree.semantics_debug_dump().contains("old child"));

    tree.update(root, Widget::layout_builder(|_| Widget::text("new child")))
        .unwrap();
    tree.layout(constraints);
    tree.update_semantics();

    let semantics = tree.semantics_debug_dump();
    assert!(semantics.contains("new child"));
    assert!(!semantics.contains("old child"));
}

#[test]
fn plain_text_uses_the_documented_natural_default_style() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(Text::new("hello").into()).unwrap();
    tree.layout(Constraints::loose(Size::new(400., 100.)));
    let render = tree.render_id(root).unwrap();
    let size = tree.render_size(render).unwrap();
    assert!(size.width > 0.0 && size.width < 100.0);
    assert!(size.height >= 16.0);
}

#[test]
fn icons_fit_and_center_their_declared_logical_box() {
    let icon: Widget = Icon::new(icons::check()).size(12.).into();
    let RenderKind::Shape { path, desired, .. } = render_kind(&icon, None) else {
        panic!("Icon should retain a shape render kind");
    };
    let bounds = path.bounds().expect("check path has geometry");
    assert_eq!(desired, Size::new(12., 12.));
    assert!(bounds.origin.x.abs() < 0.001);
    assert!(bounds.size.width <= 12.001);
    assert!(bounds.size.height <= 12.001);
    assert!((bounds.origin.x + bounds.size.width - 12.).abs() < 0.001);
    assert!((bounds.origin.y + bounds.size.height * 0.5 - 6.).abs() < 0.001);
}

#[test]
fn stateful_layout_builder_rebuilds_when_local_revision_changes() {
    let builds = Rc::new(Cell::new(0));
    let revision = Rc::new(Cell::new(0));
    let observed = builds.clone();
    let state = revision.clone();
    let builder_state = state.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::stateful_layout_builder(revision, move |_| {
            observed.set(observed.get() + 1);
            Widget::fixed_box(
                Size::new(10. + builder_state.get() as f32, 10.),
                Color::WHITE,
            )
        }))
        .unwrap();
    let constraints = Constraints::loose(Size::new(80., 20.));
    tree.layout(constraints);
    assert_eq!(builds.get(), 1);
    assert_eq!(
        tree.render_size(tree.render_id(tree.children(root).unwrap()[0]).unwrap()),
        Some(Size::new(10., 10.))
    );
    state.set(3);
    tree.layout(constraints);
    assert_eq!(builds.get(), 2);
    assert_eq!(
        tree.render_size(tree.render_id(tree.children(root).unwrap()[0]).unwrap()),
        Some(Size::new(13., 10.))
    );
}

#[test]
fn affine_transform_uses_inverse_hit_testing_and_transformed_semantics() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::transform(
            CoreTransform::translation(Offset::new(20., 10.)),
            action(Size::new(10., 10.), Color::WHITE, ActionId(1)),
        ))
        .unwrap();
    let button = tree.children(root).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let (changed, _) = tree.update_compositor(Instant::now());
    assert!(changed);
    assert_eq!(
        tree.element_for_render(tree.hit_test(Offset::new(25., 15.)).unwrap()),
        Some(button)
    );
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(button)
        .and_then(|id| tree.semantics().node(id))
        .expect("button semantics");
    assert_eq!(semantic.bounds.origin, Offset::new(20., 10.));
    assert_eq!(semantic.bounds.size, Size::new(10., 10.));
}

#[test]
fn fitted_box_scales_hits_into_the_child_coordinate_space() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::fitted_box(
            ImageFit::Contain,
            Alignment::CENTER,
            Widget::box_(Size::new(10., 20.), Color::WHITE),
        ))
        .unwrap();
    let child = tree.children(root).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let _ = tree.update_compositor(Instant::now());
    assert_eq!(
        tree.element_for_render(tree.hit_test(Offset::new(30., 10.)).unwrap()),
        Some(child)
    );
    assert!(tree.hit_test(Offset::new(20., 10.)).is_none());
}

#[test]
fn retained_text_honors_wrap_line_limit_overflow_and_rich_text_conversion() {
    let mut tree = WidgetTree::new();
    let label = tree
        .mount(
            Text::new("one two three four five six seven")
                .style(TextStyle::default().font_size(18.))
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis)
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(75., 30.)));
    let render = tree.render_id(label).unwrap();
    let layout = tree.text_layout(render).expect("text layout");
    assert_eq!(layout.lines.len(), 1);
    assert!(layout.overflowed);

    let rich: Widget = RichText::new(incular_text::TextSpan::new("one two three four"))
        .max_lines(Some(1))
        .overflow(TextOverflow::Clip)
        .into();
    let rich_label = tree.mount(rich).unwrap();
    tree.layout(Constraints::tight(Size::new(60., 30.)));
    let rich_render = tree.render_id(rich_label).unwrap();
    assert!(
        tree.text_layout(rich_render)
            .expect("text layout")
            .overflowed
    );
}

#[test]
fn scale_and_rotation_transitions_update_only_retained_compositor_layers() {
    let scale = ScaleController::new();
    let rotation = RotationController::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        RotationTransition::new(
            rotation.clone(),
            ScaleTransition::new(
                scale.clone(),
                Widget::box_(Size::new(20., 20.), Color::WHITE),
            ),
        )
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(80., 80.)));
    let _ = tree.paint();
    let before = tree.diagnostics();
    scale.set_scale(1.5);
    rotation.set_radians(0.25);
    assert!(tree.update_compositor(Instant::now()).0);
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
}

#[test]
fn rotation_transition_quarter_turn_uses_compositor_transform() {
    let controller = RotationController::new();
    assert!(controller.set_turns(0.25));
    assert!((controller.turns() - 0.25).abs() < 0.0001);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            RotationTransition::new(controller, Widget::box_(Size::new(20., 10.), Color::WHITE))
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(80., 80.)));
    let render = tree.render_id(root).unwrap();
    let transform = tree.content_transform(render).expect("rotation transform");
    let bounds =
        transform.transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, Size::new(20., 10.)));
    assert!((bounds.size.width - 10.).abs() < 0.001);
    assert!((bounds.size.height - 20.).abs() < 0.001);
}

#[test]
fn rotation_transition_alignment_changes_pivot_and_hit_testing_follows_it() {
    let center = RotationController::new();
    center.set_turns(0.25);
    let top_left = RotationController::new();
    top_left.set_turns(0.25);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::row(vec![
            RotationTransition::new(center, Widget::box_(Size::new(20., 10.), Color::WHITE)).into(),
            RotationTransition::new(top_left, Widget::box_(Size::new(20., 10.), Color::WHITE))
                .alignment(Alignment::TOP_LEFT)
                .into(),
        ]))
        .unwrap();
    let rotations = tree.children(root).expect("row children");
    let centered = rotations[0];
    let aligned = rotations[1];
    tree.layout(Constraints::tight(Size::new(80., 80.)));
    let centered_transform = tree
        .content_transform(tree.render_id(centered).unwrap())
        .expect("centered transform");
    let aligned_transform = tree
        .content_transform(tree.render_id(aligned).unwrap())
        .expect("aligned transform");
    assert_ne!(
        centered_transform.translation_offset(),
        aligned_transform.translation_offset()
    );

    let child = tree.children(aligned).expect("rotation child")[0];
    let aligned_bounds = aligned_transform
        .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, Size::new(20., 10.)));
    let hit_point = tree.render_origin(tree.render_id(aligned).unwrap())
        + aligned_bounds.origin
        + Offset::new(
            aligned_bounds.size.width * 0.5,
            aligned_bounds.size.height * 0.5,
        );
    let hit = tree.hit_test(hit_point).expect("rotated hit");
    assert_eq!(tree.element_for_render(hit), Some(child));
}

#[test]
fn replacement_opacity_controller_keeps_a_controlled_transition_alive() {
    let old_controller = OpacityController::new();
    old_controller.set_opacity(0.);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Opacity::controlled(
                old_controller,
                Widget::box_(Size::new(20., 20.), Color::WHITE),
            )
            .into(),
        )
        .unwrap();
    let constraints = Constraints::tight(Size::new(40., 40.));
    let start = Instant::now();
    tree.layout(constraints);
    tree.update_compositor(start);

    let replacement = OpacityController::new();
    tree.update(
        root,
        Opacity::controlled(
            replacement.clone(),
            Widget::box_(Size::new(20., 20.), Color::WHITE),
        )
        .into(),
    )
    .unwrap();
    assert!(replacement.opacity() < 0.01);
    tree.update_compositor(start + Duration::from_millis(70));
    assert!(replacement.opacity() > 0.01 && replacement.opacity() < 0.99);
}

#[test]
fn nested_layout_and_paint_cache_are_incremental() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::padding(
            EdgeInsets::all(2.),
            Widget::box_(Size::new(10., 5.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(20., 20.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(20., 20.))
    );
    let first = tree.paint();
    let paints = tree.diagnostics().paints;
    let _ = tree.paint();
    assert_eq!(tree.diagnostics().paints, paints);
    assert!(!first.is_empty());
}

#[test]
fn constrained_box_tightens_child_bounds_without_escaping_the_parent() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::constrained(
            Constraints::new(10., 15., 6., 12.),
            Widget::fixed_box(Size::new(5., 20.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::loose(Size::new(20., 20.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(10., 12.))
    );

    tree.layout(Constraints::tight(Size::new(8., 8.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(8., 8.))
    );
}

#[test]
fn unconstrained_box_uses_natural_child_size_but_stays_parent_bounded() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::unconstrained(
            None,
            Widget::fixed_box(Size::new(30., 5.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::loose(Size::new(20., 20.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(20., 5.))
    );
    let child = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_size(tree.render_id(child).unwrap()),
        Some(Size::new(30., 5.))
    );
}

#[test]
fn wrap_starts_a_new_run_when_a_child_exceeds_the_remaining_main_axis() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::wrap(
            Axis::Horizontal,
            2.,
            3.,
            vec![
                Widget::fixed_box(Size::new(8., 4.), Color::WHITE),
                Widget::fixed_box(Size::new(8., 6.), Color::WHITE),
            ],
        ))
        .unwrap();
    tree.layout(Constraints::loose(Size::new(17., 20.)));
    let children = tree.children(root).unwrap();
    assert_eq!(
        tree.render_origin(tree.render_id(children[0]).unwrap()),
        Offset::ZERO
    );
    assert_eq!(
        tree.render_origin(tree.render_id(children[1]).unwrap()),
        Offset::new(0., 7.)
    );
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(8., 13.))
    );
}

#[test]
fn fractional_box_tightens_requested_axes_to_parent_factors() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::fractionally_sized(
            Some(0.5),
            Some(0.25),
            Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(40., 20.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(40., 20.))
    );
    let child = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_size(tree.render_id(child).unwrap()),
        Some(Size::new(20., 5.))
    );
}

#[test]
fn table_uses_max_content_cell_sizes_and_row_major_offsets() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::table(
            2,
            2.,
            3.,
            vec![
                Widget::fixed_box(Size::new(10., 4.), Color::WHITE),
                Widget::fixed_box(Size::new(5., 8.), Color::WHITE),
                Widget::fixed_box(Size::new(7., 6.), Color::WHITE),
            ],
        ))
        .unwrap();
    tree.layout(Constraints::loose(Size::new(40., 40.)));
    let children = tree.children(root).unwrap();
    assert_eq!(
        tree.render_origin(tree.render_id(children[1]).unwrap()),
        Offset::new(12., 0.)
    );
    assert_eq!(
        tree.render_origin(tree.render_id(children[2]).unwrap()),
        Offset::new(0., 11.)
    );
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(17., 17.))
    );
}

#[test]
fn baseline_offsets_a_child_using_its_bottom_as_the_default_baseline() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::baseline(
            10.,
            Widget::fixed_box(Size::new(8., 5.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::loose(Size::new(20., 20.)));
    let child = tree.children(root).unwrap()[0];
    assert_eq!(
        tree.render_origin(tree.render_id(child).unwrap()),
        Offset::new(0., 5.)
    );
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(8., 10.))
    );
}

#[test]
fn merged_transition_composes_retained_fade_and_slide_layers() {
    let opacity = OpacityController::new();
    let translation = TranslationController::new();
    let _: Widget = Transition::new(Widget::fixed_box(Size::new(1., 1.), Color::WHITE))
        .fade(opacity)
        .slide(translation)
        .into();
}

#[test]
fn custom_paint_replays_its_display_list_in_the_retained_picture() {
    let mut display_list = DisplayList::new();
    display_list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 3.)),
        color: Color::WHITE,
    });
    let mut tree = WidgetTree::new();
    tree.mount(Widget::custom_paint(Size::new(10., 8.), display_list))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(10., 8.)));
    assert!(tree.paint().commands().iter().any(|command| {
        matches!(command, PaintCommand::Rect { rect, .. } if rect.size == Size::new(4., 3.))
    }));
}

#[test]
fn repaint_boundary_keeps_its_picture_when_child_custom_paint_changes() {
    let list = |color| {
        let mut list = DisplayList::new();
        list.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 3.)),
            color,
        });
        list
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::repaint_boundary(Widget::custom_paint(
            Size::new(10., 8.),
            list(Color::WHITE),
        )))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(10., 8.)));
    let _ = tree.paint();
    let before = tree.diagnostics().paints;
    tree.update(
        root,
        Widget::repaint_boundary(Widget::custom_paint(Size::new(10., 8.), list(Color::BLACK))),
    )
    .unwrap();
    let _ = tree.paint();
    assert_eq!(tree.diagnostics().paints, before + 1);
}

#[test]
fn stack_aligns_children_and_hits_the_frontmost_child() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::stack(
            Alignment::CENTER,
            vec![
                Widget::fixed_box(Size::new(20., 20.), Color::BLACK),
                action(Size::new(10., 10.), Color::WHITE, ActionId(1)),
            ],
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(20., 20.)));
    let children = tree.children(root).unwrap();
    let front = children[1];
    assert_eq!(
        tree.render_origin(tree.render_id(front).unwrap()),
        Offset::new(5., 5.)
    );
    assert_eq!(
        tree.element_for_render(tree.hit_test(Offset::new(10., 10.)).unwrap()),
        Some(front)
    );
}

#[test]
fn invisible_widgets_skip_child_layout_hit_testing_and_semantics() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::visibility(
            false,
            action(Size::new(20., 20.), Color::WHITE, ActionId(1)),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(20., 20.)));
    tree.update_semantics();
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(20., 20.))
    );
    assert_eq!(tree.hit_test(Offset::new(10., 10.)), None);
    assert_eq!(tree.semantics().len(), 0);
}

#[test]
fn aspect_ratio_uses_the_largest_fitting_box() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::aspect_ratio(
            2.,
            Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
        ))
        .unwrap();
    tree.layout(Constraints::loose(Size::new(100., 80.)));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(100., 50.))
    );
}

#[test]
fn hit_test_uses_reverse_paint_order_and_nested_offsets() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::padding(
            EdgeInsets::all(2.),
            Widget::row(vec![
                Widget::box_(Size::new(10., 10.), Color::WHITE),
                Widget::box_(Size::new(10., 10.), Color::BLACK),
            ]),
        ))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(30., 20.)));
    let hit = tree.hit_test(Offset::new(13., 5.)).unwrap();
    let row = tree.children(root).unwrap()[0];
    let second = tree.children(row).unwrap()[1];
    assert_eq!(tree.element_for_render(hit), Some(second));
}

#[test]
fn retained_pictures_apply_column_row_and_padding_offsets_once() {
    let mut tree = WidgetTree::new();
    tree.mount(Widget::padding(
        EdgeInsets {
            left: 20.,
            top: 10.,
            right: 0.,
            bottom: 0.,
        },
        Widget::column(vec![
            Widget::box_(Size::new(100., 20.), Color::WHITE),
            Widget::row(vec![
                Widget::box_(Size::new(30., 30.), Color::WHITE),
                Widget::box_(Size::new(40., 30.), Color::WHITE),
            ]),
            Widget::box_(Size::new(100., 40.), Color::WHITE),
        ]),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)));
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.x < 80.)
            .collect::<Vec<_>>(),
        vec![
            Offset::new(20., 10.),
            Offset::new(20., 30.),
            Offset::new(50., 30.),
            Offset::new(20., 60.),
        ]
    );
}

#[test]
fn retained_text_pictures_keep_independent_column_origins() {
    let style = TextStyle {
        size: 20.,
        line_height: Some(incular_text::LineHeight::Absolute(30.)),
        ..TextStyle::default()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::column(vec![
            Widget::text_styled("A", style.clone(), TextAlign::Start),
            Widget::text_styled("B", style.clone(), TextAlign::Start),
            Widget::text_styled("C", style, TextAlign::Start),
        ]))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)));
    let origins = glyph_origins(&tree.paint());
    assert_eq!(origins.len(), 3);
    let children = tree.children(root).unwrap();
    let a_height = tree
        .render_size(tree.render_id(children[0]).unwrap())
        .unwrap()
        .height;
    let b_height = tree
        .render_size(tree.render_id(children[1]).unwrap())
        .unwrap()
        .height;
    assert_eq!(origins[1].y - origins[0].y, a_height);
    assert_eq!(origins[2].y - origins[1].y, b_height);
    assert!(origins[0].y < origins[1].y && origins[1].y < origins[2].y);
}

#[test]
fn button_label_receives_the_button_parent_placement() {
    let mut tree = WidgetTree::new();
    tree.mount(Widget::column(vec![
        Widget::box_(Size::new(100., 30.), Color::BLACK),
        ActionSurface::new("Placed label")
            .color(Color::rgba(70, 120, 220, 255))
            .into(),
    ]))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 120.)));
    let list = tree.paint();
    let button_origin = rrect_origins(&list)[0];
    let label_origin = glyph_origins(&list)[0];
    assert_eq!(button_origin.y, 30.);
    assert!(label_origin.y >= button_origin.y);
}

#[test]
fn compositional_button_keeps_configured_size_and_content_semantics() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            ActionSurface::new("Inspector row")
                .size(Size::new(180., 34.))
                .content(Widget::row([
                    Widget::text("Inspector"),
                    Widget::text("row"),
                ]))
                .into(),
        )
        .unwrap();

    tree.layout(Constraints::new(0., 300., 0., 100.));
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(180., 34.))
    );

    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(root)
        .and_then(|id| tree.semantics().node(id))
        .expect("button semantics");
    assert_eq!(semantic.role, SemanticRole::Button);
    assert_eq!(semantic.label.as_deref(), Some("Inspector row"));
}

#[test]
fn scroll_and_animation_compose_with_static_layout_placement() {
    let scroll = ScrollController::new();
    let translation = TranslationController::new();
    translation.set_offset(Offset::new(15., 0.));
    let mut tree = WidgetTree::new();
    tree.mount(Widget::padding(
        EdgeInsets {
            left: 0.,
            top: 10.,
            right: 0.,
            bottom: 0.,
        },
        Widget::scroll_view(
            scroll.clone(),
            Widget::translate(
                translation.clone(),
                Widget::column(vec![
                    Widget::box_(Size::new(40., 20.), Color::WHITE),
                    Widget::box_(Size::new(40., 30.), Color::WHITE),
                ]),
            ),
        ),
    ))
    .unwrap();
    let constraints = Constraints::tight(Size::new(100., 40.));
    tree.layout(constraints);
    let _ = tree.update_compositor(Instant::now());
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.x < 80.)
            .collect::<Vec<_>>(),
        vec![Offset::new(15., 10.), Offset::new(15., 30.)]
    );
    assert!(scroll.jump_to(10.));
    let _ = tree.update_compositor(Instant::now());
    assert_eq!(
        rect_origins(&tree.paint())
            .into_iter()
            .filter(|origin| origin.x < 80.)
            .collect::<Vec<_>>(),
        vec![Offset::new(15., 0.), Offset::new(15., 20.)]
    );
}

#[test]
fn transparent_opacity_keeps_hit_testing_and_semantics() {
    let mut tree = WidgetTree::new();
    tree.mount(Widget::opacity(
        0.,
        ActionSurface::new("Still active").into(),
    ))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(140., 60.)));
    tree.update_semantics();
    assert!(tree.hit_test(Offset::new(10., 10.)).is_some());
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.role == SemanticRole::Button)
    );
}

#[test]
fn effect_parameter_animation_is_compositor_only_and_keeps_semantics() {
    let blur = BlurController::new(2.);
    let mut tree = WidgetTree::new();
    tree.mount(Blur::controlled(blur.clone(), ActionSurface::new("Still active")).into())
        .unwrap();
    tree.layout(Constraints::tight(Size::new(140., 60.)));
    let _ = tree.paint();
    let paints = tree.diagnostics().paints;
    let _ = tree.update_compositor(Instant::now());
    assert!(blur.set_sigma(14.));
    let (changed, _) = tree.update_compositor(Instant::now());
    assert!(changed);
    let list = tree.paint();
    assert!(list.commands().iter().any(|command| {
        matches!(command, PaintCommand::PushBlur { blur, .. } if blur.sigma_x == 14.)
    }));
    assert_eq!(tree.diagnostics().paints, paints);
    tree.update_semantics();
    assert!(tree.hit_test(Offset::new(10., 10.)).is_some());
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.role == SemanticRole::Button)
    );
}

#[test]
fn text_picture_replacement_and_root_unmount_do_not_leak_layers() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(Widget::text("Count: 0")).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)));
    let _ = tree.paint();
    let before = tree.compositor_diagnostics().layers;
    tree.update(root, Widget::text("Count: 1")).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)));
    let _ = tree.paint();
    assert_eq!(tree.compositor_diagnostics().layers, before);
    tree.update(root, Widget::text("Count: 2")).unwrap();
    tree.layout(Constraints::tight(Size::new(100., 40.)));
    let _ = tree.paint();
    assert_eq!(tree.compositor_diagnostics().layers, before);
    tree.mount(Widget::box_(Size::new(1., 1.), Color::WHITE))
        .unwrap();
    assert_eq!(tree.compositor_diagnostics().layers, 2);
}

#[test]
fn color_filter_and_blend_updates_stay_in_the_retained_compositor() {
    let controller = ColorFilterController::new(ColorFilter::grayscale(0.));
    let widget = ColorFiltered::controlled(
        controller.clone(),
        Blend::new(
            BlendMode::Multiply,
            Widget::box_(Size::new(80., 40.), Color::rgba(200, 80, 40, 255)),
        ),
    );
    let mut tree = WidgetTree::new();
    tree.mount(widget.into()).expect("mount effect tree");
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let _ = tree.paint();
    let before = tree.diagnostics();
    assert!(controller.set_filter(ColorFilter::sepia(1.)));
    let (changed, _) = tree.update_compositor(Instant::now());
    let after = tree.diagnostics();
    assert!(changed);
    assert_eq!(after.paints, before.paints);
    assert!(after.composites > before.composites);
    let debug = tree.compositor_debug_tree();
    assert!(debug.contains("ColorFilter"));
    assert!(debug.contains("Blend"));
}

#[test]
fn replacing_effect_families_releases_the_old_attachment_subtree() {
    fn effect(index: usize) -> Widget {
        let child = Widget::box_(Size::new(20., 20.), Color::WHITE);
        match index % 5 {
            0 => Widget::opacity(0.5, child),
            1 => Widget::blur(4., child),
            2 => Widget::drop_shadow(Offset::new(2., 3.), 5., Color::rgba(0, 0, 0, 120), child),
            3 => Widget::color_filtered(ColorFilter::sepia(0.75), child),
            _ => Widget::blend(BlendMode::Multiply, child),
        }
        .with_key(incular_widgets::internal::Key::Value(7))
    }

    fn root(index: usize) -> Widget {
        Widget::column(vec![effect(index)])
    }

    let mut tree = WidgetTree::new();
    let root_id = tree.mount(root(0)).expect("mount effect root");
    tree.layout(Constraints::tight(Size::new(100., 100.)));
    let _ = tree.paint();
    let stable_layers = tree.compositor_diagnostics().layers;

    for index in 1..100 {
        tree.update(root_id, root(index))
            .expect("replace keyed effect");
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let _ = tree.paint();
        assert_eq!(
            tree.compositor_diagnostics().layers,
            stable_layers,
            "effect replacement leaked a retained attachment at iteration {index}"
        );
    }

    tree.mount(Widget::box_(Size::new(1., 1.), Color::WHITE))
        .expect("replace root");
    assert_eq!(tree.compositor_diagnostics().layers, 2);
}
