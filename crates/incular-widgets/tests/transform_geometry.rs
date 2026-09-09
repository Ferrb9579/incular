//! Geometry contract for retained transforms: one shared resolution
//! across paint, hit testing, and semantics.
//!
//! Every case mounts, lays out, and paints first, then updates and
//! re-runs the phases: painted geometry comes from `content_transform`,
//! hits must land inside the new location and miss the old one, semantic
//! bounds must follow, element/render identity must hold, and phase
//! counters must show the documented invalidation (presentation
//! transforms are compositor-only; `RotatedBox` is layout-neutral by
//! construction and rotates at the compositor like every `Transform`).
//! Asymmetric 40x20 fixtures keep incorrect origins and axis swaps
//! observable. Singular and non-finite transforms are documented policy:
//! `inverse_transform_point` returns `None`, so hits miss while paint
//! and semantics keep their cached values.

mod common;

use common::*;
use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size, Transform as CoreTransform};
use incular_rendering::{DisplayList, PaintCommand};
use incular_widgets::internal::*;
use incular_widgets::{FractionalTranslation, RotatedBox, Transform, Widget};
use std::time::Instant;

const CHILD: (f32, f32) = (40., 20.);

fn child_box() -> Widget {
    Widget::box_(Size::new(CHILD.0, CHILD.1), Color::WHITE)
}

fn button() -> Widget {
    action(Size::new(CHILD.0, CHILD.1), Color::WHITE, ActionId(1))
        .accessibility_label("moved child")
}

fn layout_paint(tree: &mut WidgetTree) {
    let constraints = Constraints::tight(Size::new(200., 200.));
    tree.layout(constraints).expect("layout");
    let _ = tree.paint();
}

fn approx_eq(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.1,
        "{what}: expected {expected}, got {actual}"
    );
}

#[test]
fn translation_update_is_compositor_only() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Transform::translation(Offset::new(4., 2.), button()).into())
        .unwrap();
    let moved = tree.children(root).unwrap()[0];
    layout_paint(&mut tree);
    let before_paint = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        Transform::translation(Offset::new(20., 10.), button()).into(),
    )
    .expect("update");
    layout_paint(&mut tree);
    assert_eq!(tree.children(root).unwrap()[0], moved);
    let after = tree.diagnostics();
    assert_eq!(
        after.layouts, before.layouts,
        "translation must not relayout"
    );
    assert_eq!(after.paints, before.paints, "translation must not repaint");
    assert_eq!(
        after.compositor_only_updates - before.compositor_only_updates,
        1
    );
    // The cached child picture is reused verbatim; only the cheap
    // wrapping translation command reflects the new offset.
    fn child_commands(list: &DisplayList) -> Vec<PaintCommand> {
        list.commands()
            .iter()
            .filter(|command| {
                !matches!(
                    command,
                    PaintCommand::PushTransform { .. } | PaintCommand::PopTransform
                )
            })
            .cloned()
            .collect()
    }
    assert_eq!(child_commands(&tree.paint()), child_commands(&before_paint));
    let hit = tree
        .hit_test(Offset::new(25., 15.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("moved semantics");
    assert_eq!(semantic.bounds.origin, Offset::new(20., 10.));
    assert_eq!(semantic.bounds.size, Size::new(CHILD.0, CHILD.1));
}

#[test]
fn scale_about_default_center_moves_geometry() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(Transform::scale(1., button()).into()).unwrap();
    let moved = tree.children(root).unwrap()[0];
    layout_paint(&mut tree);
    tree.update(root, Transform::scale(2., button()).into())
        .expect("update");
    layout_paint(&mut tree);
    assert_eq!(tree.children(root).unwrap()[0], moved);
    let transform = tree
        .content_transform(tree.render_id(root).unwrap())
        .expect("scale transform");
    let bounds = transform.transform_rect_bbox(Rect::from_origin_size(
        Offset::ZERO,
        Size::new(CHILD.0, CHILD.1),
    ));
    approx_eq(bounds.origin.x, -20., "scale bbox x");
    approx_eq(bounds.origin.y, -10., "scale bbox y");
    approx_eq(bounds.size.width, 80., "scale bbox width");
    approx_eq(bounds.size.height, 40., "scale bbox height");
    let hit = tree
        .hit_test(Offset::new(0., 0.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    assert!(tree.hit_test(Offset::new(70., 25.)).is_none());
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("scaled semantics");
    approx_eq(semantic.bounds.origin.x, -20., "semantic x");
    approx_eq(semantic.bounds.size.width, 80., "semantic width");
}

#[test]
fn rotation_quarter_turn_about_default_center() {
    use std::f32::consts::FRAC_PI_2;
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Transform::new(CoreTransform::IDENTITY, button()).into())
        .unwrap();
    let moved = tree.children(root).unwrap()[0];
    layout_paint(&mut tree);
    let before = tree.diagnostics();
    tree.update(root, Transform::rotation(FRAC_PI_2, button()).into())
        .expect("update");
    layout_paint(&mut tree);
    assert_eq!(tree.children(root).unwrap()[0], moved);
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts, "rotation must not relayout");
    assert_eq!(after.paints, before.paints, "rotation must not repaint");
    let transform = tree
        .content_transform(tree.render_id(root).unwrap())
        .expect("rotation transform");
    let bounds = transform.transform_rect_bbox(Rect::from_origin_size(
        Offset::ZERO,
        Size::new(CHILD.0, CHILD.1),
    ));
    approx_eq(bounds.size.width, 20., "rotated width");
    approx_eq(bounds.size.height, 40., "rotated height");
    approx_eq(bounds.origin.x, 10., "rotated x");
    approx_eq(bounds.origin.y, -10., "rotated y");
    // Inside the rotated (20x40) location hits; the vacated corner misses.
    let hit = tree
        .hit_test(Offset::new(20., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    assert!(tree.hit_test(Offset::new(35., 5.)).is_none());
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("rotated semantics");
    approx_eq(semantic.bounds.size.width, 20., "semantic width");
    approx_eq(semantic.bounds.size.height, 40., "semantic height");
}

#[test]
fn nonzero_origin_changes_the_pivot() {
    use std::f32::consts::FRAC_PI_2;
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Transform::rotation(FRAC_PI_2, child_box()).into())
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let centered = tree
        .content_transform(tree.render_id(root).unwrap())
        .expect("centered transform");
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Transform::new(CoreTransform::rotation(FRAC_PI_2), child_box())
                .origin(Offset::ZERO)
                .into(),
        )
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let corner = tree
        .content_transform(tree.render_id(root).unwrap())
        .expect("corner transform");
    let centered_box = centered.transform_rect_bbox(Rect::from_origin_size(
        Offset::ZERO,
        Size::new(CHILD.0, CHILD.1),
    ));
    let corner_box = corner.transform_rect_bbox(Rect::from_origin_size(
        Offset::ZERO,
        Size::new(CHILD.0, CHILD.1),
    ));
    approx_eq(centered_box.size.width, 20., "centered width");
    approx_eq(corner_box.size.width, 20., "corner width");
    approx_eq(corner_box.origin.x, -20., "corner pivot x");
    approx_eq(corner_box.origin.y, 0., "corner pivot y");
    assert!(
        (centered_box.origin.x - corner_box.origin.x).abs() > 1.
            || (centered_box.origin.y - corner_box.origin.y).abs() > 1.,
        "origin must move the pivot: centered {centered_box:?}, corner {corner_box:?}"
    );
}

#[test]
fn fractional_translation_scales_by_child_size() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(FractionalTranslation::new(Offset::new(0.5, 0.5), button()).into())
        .unwrap();
    let moved = tree.children(root).unwrap()[0];
    layout_paint(&mut tree);
    let transform = tree
        .content_transform(tree.render_id(root).unwrap())
        .expect("fraction transform");
    assert_eq!(transform.translation_offset(), Offset::new(20., 10.));
    // Painted at x 20..60, y 10..30 but laid out at 0..40, 0..20:
    // hits follow paint.
    let hit = tree
        .hit_test(Offset::new(50., 15.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("fraction semantics");
    assert_eq!(semantic.bounds.origin, Offset::new(20., 10.));
    assert_eq!(semantic.bounds.size, Size::new(CHILD.0, CHILD.1));
}

#[test]
fn fractional_translation_hit_tests_flag_selects_coordinate_space() {
    for converts in [true, false] {
        let mut tree = WidgetTree::new();
        let widget = FractionalTranslation::new(Offset::new(0.5, 0.), button())
            .transform_hit_tests(converts);
        let root = tree.mount(widget.into()).unwrap();
        let moved = tree.children(root).unwrap()[0];
        layout_paint(&mut tree);
        tree.update_compositor(Instant::now()).expect("compositor");
        let painted_only = tree
            .hit_test(Offset::new(50., 5.))
            .and_then(|render| tree.element_for_render(render));
        let layout_only = tree
            .hit_test(Offset::new(5., 5.))
            .and_then(|render| tree.element_for_render(render));
        if converts {
            assert_eq!(painted_only, Some(moved));
            assert_eq!(layout_only, None);
        } else {
            // Hits land in layout space: the button answers at its laid
            // out position, while the painted-only point never reaches it
            // through the transform.
            assert_eq!(layout_only, Some(moved));
            assert_ne!(painted_only, Some(moved));
        }
        // Painting and semantics always follow the visual transform.
        tree.update_semantics();
        let semantic = tree
            .semantic_node_for_element(moved)
            .and_then(|id| tree.semantics().node(id))
            .expect("flag semantics");
        assert_eq!(semantic.bounds.origin, Offset::new(20., 0.));
    }
}

#[test]
fn rotated_box_keeps_layout_size_and_rotates_geometry() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(RotatedBox::new(1, button()).into()).unwrap();
    let rotated = tree.children(root).unwrap()[0];
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    // Layout-neutral by construction: the rotation applies at the
    // compositor exactly like an equivalent presentation transform.
    assert_eq!(
        tree.render_size(tree.render_id(root).unwrap()),
        Some(Size::new(CHILD.0, CHILD.1))
    );
    let transform = tree
        .content_transform(tree.render_id(root).unwrap())
        .expect("rotated transform");
    let bounds = transform.transform_rect_bbox(Rect::from_origin_size(
        Offset::ZERO,
        Size::new(CHILD.0, CHILD.1),
    ));
    approx_eq(bounds.size.width, 20., "rotated width");
    approx_eq(bounds.size.height, 40., "rotated height");
    let hit = tree
        .hit_test(Offset::new(20., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(rotated));
    assert!(tree.hit_test(Offset::new(35., 5.)).is_none());
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(rotated)
        .and_then(|id| tree.semantics().node(id))
        .expect("rotated semantics");
    approx_eq(semantic.bounds.size.width, 20., "semantic width");
    approx_eq(semantic.bounds.size.height, 40., "semantic height");
}

#[test]
fn reapplying_identical_transform_schedules_no_phases() {
    let make = || {
        Transform::translation(
            Offset::new(20., 10.),
            Widget::box_(Size::new(20., 20.), Color::WHITE),
        )
        .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make()).unwrap();
    layout_paint(&mut tree);
    let before = tree.diagnostics();
    tree.update(root, make()).expect("update");
    layout_paint(&mut tree);
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
}

#[test]
fn fractional_translation_builder_matches_fluent_construction() {
    let child = Widget::box_(Size::new(10., 10.), Color::WHITE);
    assert_eq!(
        FractionalTranslation::builder()
            .translation(Offset::new(0.5, 0.25))
            .transform_hit_tests(false)
            .child(child.clone())
            .build(),
        FractionalTranslation::new(Offset::new(0.5, 0.25), child).transform_hit_tests(false)
    );
}

#[test]
fn rotated_box_builder_matches_fluent_construction() {
    let child = Widget::box_(Size::new(10., 10.), Color::WHITE);
    assert_eq!(
        RotatedBox::builder()
            .quarter_turns(3)
            .child(child.clone())
            .build(),
        RotatedBox::new(3, child)
    );
}
