//! Utility-family coverage: SafeArea, SplitView, OverflowBar.
//!
//! SafeArea consumes the ambient [`RuntimeEnvironment`] insets; SplitView is a
//! stateless composite over Row/Column with a draggable divider; OverflowBar
//! is a thin facade lowering directly to Wrap.

use incular_config::{Constraints, EdgeInsets, RuntimeEnvironment, WrapCrossAlignment};
use incular_core::{Color, Offset, PointerPhase, Size};
use incular_gestures::PointerEvent;
use incular_widgets::{
    OverflowBar, SafeArea, Widget,
    internal::{ElementId, SplitPosition, SplitView, WidgetTree},
};
use std::{cell::RefCell, rc::Rc, time::Instant};

fn box_(w: f32, h: f32) -> Widget {
    Widget::box_(Size::new(w, h), Color::WHITE)
}

fn tight(tree: &mut WidgetTree, root: ElementId, w: f32, h: f32) {
    tree.layout(Constraints::tight(Size::new(w, h)))
        .expect("layout");
    let _ = root;
}

fn only_child(tree: &WidgetTree, id: ElementId) -> ElementId {
    let kids = tree.children(id).expect("element has children");
    assert_eq!(kids.len(), 1, "expected a single child");
    kids[0]
}

fn bounds(tree: &WidgetTree, id: ElementId) -> (Offset, Size) {
    let rect = tree.element_bounds(id).expect("element has bounds");
    (rect.origin, rect.size)
}

/// Builds an ambient snapshot from the three owned inset signals: the
/// currently usable margin, the persistent obstruction margin, and the
/// transient occlusion (for example the keyboard).
fn environment_with(
    safe: EdgeInsets,
    padding: EdgeInsets,
    occlusion: EdgeInsets,
) -> RuntimeEnvironment {
    RuntimeEnvironment {
        safe_insets: safe,
        view_padding: padding,
        view_insets: occlusion,
        ..RuntimeEnvironment::default()
    }
}

fn bottom_only(value: f32) -> EdgeInsets {
    EdgeInsets::only(0., 0., 0., value)
}

#[test]
fn safe_area_builder_defaults_match_new() {
    let child = box_(10., 10.);
    let built = SafeArea::builder().child(child.clone()).build();
    assert_eq!(built, SafeArea::new(child));
}

#[test]
fn safe_area_applies_all_edges() {
    let mut tree = WidgetTree::new();
    tree.set_environment(environment_with(
        EdgeInsets::only(10., 20., 30., 40.),
        EdgeInsets::ZERO,
        EdgeInsets::ZERO,
    ));
    let root = tree
        .mount(SafeArea::new(box_(50., 50.)).into())
        .expect("mount");
    tight(&mut tree, root, 200., 200.);
    let (origin, size) = bounds(&tree, only_child(&tree, root));
    assert_eq!(origin, Offset::new(10., 20.));
    // The box fills the padded area, proving the padding was subtracted.
    assert_eq!(size, Size::new(160., 140.));
}

#[test]
fn safe_area_disabled_edges_fall_to_minimum() {
    let mut tree = WidgetTree::new();
    tree.set_environment(environment_with(
        EdgeInsets::only(10., 20., 30., 40.),
        EdgeInsets::ZERO,
        EdgeInsets::ZERO,
    ));
    let root = tree
        .mount(
            SafeArea::new(box_(50., 50.))
                .sides(false, true, false, true)
                .minimum(EdgeInsets::all(5.))
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 200.);
    let (origin, _) = bounds(&tree, only_child(&tree, root));
    assert_eq!(origin, Offset::new(5., 20.));
}

#[test]
fn safe_area_minimum_raises_padding() {
    let mut tree = WidgetTree::new();
    tree.set_environment(environment_with(
        EdgeInsets::only(10., 20., 30., 40.),
        EdgeInsets::ZERO,
        EdgeInsets::ZERO,
    ));
    let root = tree
        .mount(
            SafeArea::new(box_(50., 50.))
                .minimum(EdgeInsets::all(50.))
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 200.);
    let (origin, _) = bounds(&tree, only_child(&tree, root));
    assert_eq!(origin, Offset::new(50., 50.));
}

#[test]
fn safe_area_nested_accumulates() {
    let mut tree = WidgetTree::new();
    tree.set_environment(environment_with(
        EdgeInsets::only(10., 20., 30., 40.),
        EdgeInsets::ZERO,
        EdgeInsets::ZERO,
    ));
    let root = tree
        .mount(SafeArea::new(SafeArea::new(box_(50., 50.))).into())
        .expect("mount");
    tight(&mut tree, root, 300., 300.);
    let inner = only_child(&tree, root);
    let (origin, size) = bounds(&tree, only_child(&tree, inner));
    assert_eq!(origin, Offset::new(20., 40.));
    // Both padding layers subtract: 300 - 2*(10+30) by 300 - 2*(20+40).
    assert_eq!(size, Size::new(220., 180.));
}

// A device with a 20px persistent home indicator and a 180px keyboard.
// While the keyboard is shown the shell reports the usable bottom margin
// as zero but keeps the persistent margin at 20.
const PERSISTENT_BOTTOM: f32 = 20.;
const KEYBOARD_HEIGHT: f32 = 180.;
const FRAME: f32 = 200.;

fn keyboard_hidden() -> RuntimeEnvironment {
    environment_with(
        bottom_only(PERSISTENT_BOTTOM),
        bottom_only(PERSISTENT_BOTTOM),
        EdgeInsets::ZERO,
    )
}

fn keyboard_shown() -> RuntimeEnvironment {
    environment_with(
        EdgeInsets::ZERO,
        bottom_only(PERSISTENT_BOTTOM),
        bottom_only(KEYBOARD_HEIGHT),
    )
}

fn child_height(tree: &WidgetTree, root: ElementId) -> f32 {
    bounds(tree, only_child(tree, root)).1.height
}

#[test]
fn safe_area_maintain_survives_keyboard_cycle() {
    let mut maintained = WidgetTree::new();
    maintained.set_environment(keyboard_hidden());
    let kept = maintained
        .mount(
            SafeArea::new(box_(50., 50.))
                .maintain_bottom_view_padding(true)
                .into(),
        )
        .expect("mount");
    let mut plain = WidgetTree::new();
    plain.set_environment(keyboard_hidden());
    let unkept = plain
        .mount(SafeArea::new(box_(50., 50.)).into())
        .expect("mount");

    // Hidden keyboard: both read the usable 20px margin.
    tight(&mut maintained, kept, FRAME, FRAME);
    tight(&mut plain, unkept, FRAME, FRAME);
    assert_eq!(child_height(&maintained, kept), FRAME - PERSISTENT_BOTTOM);
    assert_eq!(child_height(&plain, unkept), FRAME - PERSISTENT_BOTTOM);

    // Shown keyboard: the usable margin collapses, but the maintained
    // edge keeps the persistent 20px — never the 180px occlusion.
    maintained.set_environment(keyboard_shown());
    plain.set_environment(keyboard_shown());
    tight(&mut maintained, kept, FRAME, FRAME);
    tight(&mut plain, unkept, FRAME, FRAME);
    assert_eq!(child_height(&maintained, kept), FRAME - PERSISTENT_BOTTOM);
    assert_eq!(child_height(&plain, unkept), FRAME);

    // Hidden again: both return to the usable margin.
    maintained.set_environment(keyboard_hidden());
    plain.set_environment(keyboard_hidden());
    tight(&mut maintained, kept, FRAME, FRAME);
    tight(&mut plain, unkept, FRAME, FRAME);
    assert_eq!(child_height(&maintained, kept), FRAME - PERSISTENT_BOTTOM);
    assert_eq!(child_height(&plain, unkept), FRAME - PERSISTENT_BOTTOM);
}

#[test]
fn safe_area_mounted_while_keyboard_shown_uses_snapshot() {
    // No history is involved: a tree mounted under occlusion derives the
    // same padding from the current snapshot alone.
    let mut maintained = WidgetTree::new();
    maintained.set_environment(keyboard_shown());
    let kept = maintained
        .mount(
            SafeArea::new(box_(50., 50.))
                .maintain_bottom_view_padding(true)
                .into(),
        )
        .expect("mount");
    tight(&mut maintained, kept, FRAME, FRAME);
    assert_eq!(child_height(&maintained, kept), FRAME - PERSISTENT_BOTTOM);

    let mut plain = WidgetTree::new();
    plain.set_environment(keyboard_shown());
    let unkept = plain
        .mount(SafeArea::new(box_(50., 50.)).into())
        .expect("mount");
    tight(&mut plain, unkept, FRAME, FRAME);
    assert_eq!(child_height(&plain, unkept), FRAME);
}

#[test]
fn safe_area_with_avoiding_parent_counts_occlusion_once() {
    // A Scaffold-style parent already pads the 180px occlusion; the
    // maintained SafeArea adds only its persistent 20px on top.
    for (maintain, expected) in [
        (true, 400. - KEYBOARD_HEIGHT - PERSISTENT_BOTTOM),
        (false, 400. - KEYBOARD_HEIGHT),
    ] {
        let mut tree = WidgetTree::new();
        tree.set_environment(keyboard_shown());
        let mut safe = SafeArea::new(box_(50., 50.));
        if maintain {
            safe = safe.maintain_bottom_view_padding(true);
        }
        let root = tree
            .mount(incular_widgets::Padding::new(bottom_only(KEYBOARD_HEIGHT), safe).into())
            .expect("mount");
        tight(&mut tree, root, 400., 400.);
        assert_eq!(child_height(&tree, only_child(&tree, root)), expected);
    }
}

#[test]
fn safe_area_nested_maintained_accumulates_from_snapshot() {
    // Each layer keeps the full persistent margin from the same ambient
    // snapshot; nothing is cached between layouts.
    let mut tree = WidgetTree::new();
    tree.set_environment(keyboard_shown());
    let root = tree
        .mount(
            SafeArea::new(SafeArea::new(box_(50., 50.)).maintain_bottom_view_padding(true))
                .maintain_bottom_view_padding(true)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 400., 400.);
    let inner = only_child(&tree, root);
    assert_eq!(
        bounds(&tree, only_child(&tree, inner)).1.height,
        400. - 2. * PERSISTENT_BOTTOM
    );
}

#[test]
fn safe_area_disabled_bottom_and_minimum_ignore_maintenance() {
    // A disabled bottom edge stays at its minimum even with maintenance.
    let mut tree = WidgetTree::new();
    tree.set_environment(keyboard_shown());
    let root = tree
        .mount(
            SafeArea::new(box_(50., 50.))
                .bottom(false)
                .minimum(EdgeInsets::all(5.))
                .maintain_bottom_view_padding(true)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, FRAME, FRAME);
    // The all-edges minimum pads top and bottom: 200 - 5 - 5.
    assert_eq!(child_height(&tree, root), FRAME - 10.);

    // A minimum above the persistent margin wins over maintenance.
    let mut tree = WidgetTree::new();
    tree.set_environment(keyboard_shown());
    let root = tree
        .mount(
            SafeArea::new(box_(50., 50.))
                .minimum(EdgeInsets::all(30.))
                .maintain_bottom_view_padding(true)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, FRAME, FRAME);
    // Top and bottom both resolve to the 30px minimum: 200 - 30 - 30.
    assert_eq!(child_height(&tree, root), FRAME - 60.);
}

#[test]
fn safe_area_tracks_padding_and_safe_margin_not_occlusion() {
    let mut tree = WidgetTree::new();
    tree.set_environment(keyboard_shown());
    let root = tree
        .mount(
            SafeArea::new(box_(50., 50.))
                .maintain_bottom_view_padding(true)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, FRAME, FRAME);
    assert_eq!(child_height(&tree, root), FRAME - PERSISTENT_BOTTOM);

    // Persistent margin 20 -> 30 with occlusion fixed: padding follows.
    let dirty = tree.set_environment(environment_with(
        EdgeInsets::ZERO,
        bottom_only(30.),
        bottom_only(KEYBOARD_HEIGHT),
    ));
    assert!(dirty, "view-padding change must dirty maintained SafeArea");
    tight(&mut tree, root, FRAME, FRAME);
    assert_eq!(child_height(&tree, root), FRAME - 30.);

    // Occlusion 180 -> 100 with padding fixed: padding is unaffected, and
    // the transient change alone dirties nothing in this tree.
    let dirty = tree.set_environment(environment_with(
        EdgeInsets::ZERO,
        bottom_only(30.),
        bottom_only(100.),
    ));
    assert!(!dirty, "occlusion alone must not dirty SafeArea");
    tight(&mut tree, root, FRAME, FRAME);
    assert_eq!(child_height(&tree, root), FRAME - 30.);

    // Usable margin 0 -> 40 above the persistent 30: safe margin still
    // participates in the maintained edge.
    let dirty = tree.set_environment(environment_with(
        bottom_only(40.),
        bottom_only(30.),
        bottom_only(100.),
    ));
    assert!(dirty, "safe-margin change must dirty SafeArea");
    tight(&mut tree, root, FRAME, FRAME);
    assert_eq!(child_height(&tree, root), FRAME - 40.);
}

#[test]
fn safe_area_resolve_with_padding_matches_retained() {
    // The environment-aware standalone path applies the same policy as
    // retained construction in every snapshot, including under occlusion
    // (which never participates in either path).
    for environment in [keyboard_hidden(), keyboard_shown()] {
        for maintain in [false, true] {
            let mut retained_tree = WidgetTree::new();
            retained_tree.set_environment(environment.clone());
            let retained = retained_tree
                .mount(
                    SafeArea::new(box_(50., 50.))
                        .maintain_bottom_view_padding(maintain)
                        .into(),
                )
                .expect("mount");
            tight(&mut retained_tree, retained, FRAME, FRAME);
            let retained_height = child_height(&retained_tree, retained);

            let mut resolved_tree = WidgetTree::new();
            let resolved = resolved_tree
                .mount(
                    SafeArea::new(box_(50., 50.))
                        .maintain_bottom_view_padding(maintain)
                        .resolve_with_padding(environment.safe_insets, environment.view_padding),
                )
                .expect("mount");
            tight(&mut resolved_tree, resolved, FRAME, FRAME);
            assert_eq!(
                child_height(&resolved_tree, resolved),
                retained_height,
                "maintain={maintain} shown={}",
                environment.view_insets.bottom > 0.
            );
        }
    }
}

#[test]
fn safe_area_resolve_ignores_maintenance_by_contract() {
    // The explicit-insets path has no persistent signal to keep, so the
    // flag is a documented no-op there rather than a silent half-policy.
    for maintain in [false, true] {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                SafeArea::new(box_(50., 50.))
                    .maintain_bottom_view_padding(maintain)
                    .resolve(EdgeInsets::only(0., 0., 0., 10.)),
            )
            .expect("mount");
        tight(&mut tree, root, FRAME, FRAME);
        assert_eq!(child_height(&tree, root), FRAME - 10.);
    }
}

#[test]
fn safe_area_resolve_applies_outer_level_only() {
    // Standalone resolution lowers one descriptor to Padding; a nested
    // SafeArea child stays a retained descriptor and reads whatever
    // ambient environment the mounting tree provides (zero here), so only
    // the outer 20px applies. Callers compose levels explicitly instead
    // of expecting recursive resolution of opaque children.
    let environment = keyboard_shown();
    let nested = SafeArea::new(SafeArea::new(box_(50., 50.)).maintain_bottom_view_padding(true))
        .maintain_bottom_view_padding(true);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(nested.resolve_with_padding(environment.safe_insets, environment.view_padding))
        .expect("mount");
    tight(&mut tree, root, 400., 400.);
    let mut id = root;
    while let Some([only]) = tree.children(id) {
        id = *only;
    }
    assert_eq!(bounds(&tree, id).1.height, 400. - PERSISTENT_BOTTOM);
}

#[test]
fn split_view_builder_defaults_match_constructors() {
    // SplitView carries an Rc callback so it has no structural equality;
    // parity is proven geometrically: the generated builder with defaults
    // lays out exactly like the matching constructor.
    for axis in [
        incular_config::Axis::Horizontal,
        incular_config::Axis::Vertical,
    ] {
        let mut via_builder = WidgetTree::new();
        let builder_root = via_builder
            .mount(
                SplitView::builder()
                    .axis(axis)
                    .first(box_(10., 10.))
                    .second(box_(10., 10.))
                    .build()
                    .into(),
            )
            .expect("mount");
        via_builder
            .layout(Constraints::tight(Size::new(200., 100.)))
            .expect("layout");
        let mut via_ctor = WidgetTree::new();
        let ctor_root = via_ctor
            .mount(
                (if axis == incular_config::Axis::Horizontal {
                    SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                } else {
                    SplitView::vertical(box_(10., 10.), box_(10., 10.))
                })
                .into(),
            )
            .expect("mount");
        via_ctor
            .layout(Constraints::tight(Size::new(200., 100.)))
            .expect("layout");
        let builder_kids = via_builder.children(builder_root).expect("kids").to_vec();
        let ctor_kids = via_ctor.children(ctor_root).expect("kids").to_vec();
        assert_eq!(builder_kids.len(), ctor_kids.len());
        for (left, right) in builder_kids.iter().zip(ctor_kids.iter()) {
            assert_eq!(
                via_builder
                    .element_bounds(*left)
                    .map(|r| (r.origin, r.size)),
                via_ctor.element_bounds(*right).map(|r| (r.origin, r.size))
            );
        }
    }
}

#[test]
fn split_view_horizontal_fraction_geometry() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(SplitView::horizontal(box_(10., 10.), box_(10., 10.)).into())
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    assert_eq!(kids.len(), 3);
    let (_, first_size) = bounds(&tree, kids[0]);
    let (divider_origin, divider_size) = bounds(&tree, kids[1]);
    let (second_origin, _) = bounds(&tree, kids[2]);
    // Loose panes keep intrinsic size; the 8px divider sits between them.
    assert_eq!(first_size, Size::new(10., 10.));
    assert_eq!(divider_origin, Offset::new(10., 0.));
    assert_eq!(divider_size, Size::new(8., 100.));
    assert_eq!(second_origin, Offset::new(18., 45.));
}

#[test]
fn split_view_vertical_axis_geometry() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(SplitView::vertical(box_(10., 10.), box_(10., 10.)).into())
        .expect("mount");
    tight(&mut tree, root, 100., 200.);
    let kids = tree.children(root).expect("column children").to_vec();
    assert_eq!(kids.len(), 3);
    let (divider_origin, divider_size) = bounds(&tree, kids[1]);
    let (second_origin, _) = bounds(&tree, kids[2]);
    assert_eq!(divider_origin, Offset::new(0., 10.));
    assert_eq!(divider_size, Size::new(100., 8.));
    assert_eq!(second_origin, Offset::new(45., 18.));
}

#[test]
fn split_view_first_extent_clamps_to_minimum() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .first_extent(60.)
                .min_first(80.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (_, first_size) = bounds(&tree, kids[0]);
    assert_eq!(first_size.width, 80.);
}

#[test]
fn split_view_extent_limits_clamp_both_panes() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .first_extent(190.)
                .max_first(70.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (_, first_size) = bounds(&tree, kids[0]);
    assert_eq!(first_size.width, 70.);

    // The second pane clamps through its own limits: above max and below
    // min both resolve inside [min_second, max_second].
    for (extent, expected) in [(190., 60.), (10., 20.)] {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                    .second_extent(extent)
                    .max_second(60.)
                    .min_second(20.)
                    .into(),
            )
            .expect("mount");
        tight(&mut tree, root, 200., 100.);
        let kids = tree.children(root).expect("row children").to_vec();
        let (_, second_size) = bounds(&tree, kids[2]);
        assert_eq!(second_size.width, expected);
    }
}

#[test]
fn split_view_split_offset_aliases_first_extent() {
    assert_eq!(
        divider_x(SplitView::horizontal(box_(500., 10.), box_(500., 10.)).split_offset(90.)),
        divider_x(SplitView::horizontal(box_(500., 10.), box_(500., 10.)).first_extent(90.))
    );
}

#[test]
fn split_view_divider_thickness_sets_both_extents() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .divider_thickness(4.)
                .divider_color(Color::rgba(255, 0, 0, 255))
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (divider_origin, divider_size) = bounds(&tree, kids[1]);
    assert_eq!(divider_origin, Offset::new(10., 0.));
    assert_eq!(divider_size, Size::new(4., 100.));
}

#[test]
fn split_view_from_end_positions_second_pane() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .second_extent(70.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (second_origin, second_size) = bounds(&tree, kids[2]);
    assert_eq!(second_size.width, 70.);
    assert_eq!(second_origin.x, 130.);
}

fn divider_x(split: SplitView) -> f32 {
    // Oversized panes clamp to their flex share, so the divider position
    // exposes the resolved fraction.
    let mut tree = WidgetTree::new();
    let root = tree.mount(split.into()).expect("mount");
    tight(&mut tree, root, 208., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    bounds(&tree, kids[1]).0.x
}

#[test]
fn split_view_fraction_clamps_to_unit_range() {
    let wide = || SplitView::horizontal(box_(500., 10.), box_(500., 10.));
    // 0.25 of the 200px available to panes puts the divider at 50.
    assert_eq!(divider_x(wide().split_fraction(0.25)), 50.);
    // Out-of-range fractions clamp to the matching endpoint geometry.
    assert_eq!(
        divider_x(wide().split_fraction(2.0)),
        divider_x(wide().split_fraction(1.0))
    );
    assert_eq!(
        divider_x(wide().split_fraction(-1.0)),
        divider_x(wide().split_fraction(0.0))
    );
    assert!(divider_x(wide().split_fraction(2.0)) > 199.);
    assert!(divider_x(wide().split_fraction(-1.0)) < 1.);
}

#[test]
fn split_view_unequal_panes_position_divider() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .first_extent(40.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (divider_origin, divider_size) = bounds(&tree, kids[1]);
    let (_, second_size) = bounds(&tree, kids[2]);
    assert_eq!(divider_origin, Offset::new(40., 0.));
    assert_eq!(divider_size, Size::new(8., 100.));
    assert_eq!(second_size, Size::new(152., 10.));

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::vertical(box_(10., 10.), box_(10., 10.))
                .first_extent(30.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 100., 200.);
    let kids = tree.children(root).expect("column children").to_vec();
    let (divider_origin, divider_size) = bounds(&tree, kids[1]);
    let (_, second_size) = bounds(&tree, kids[2]);
    assert_eq!(divider_origin, Offset::new(0., 30.));
    assert_eq!(divider_size, Size::new(100., 8.));
    assert_eq!(second_size, Size::new(10., 162.));
}

#[test]
fn split_view_divider_spans_cross_axis_for_hit() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(SplitView::horizontal(box_(10., 10.), box_(10., 10.)).into())
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let hit_at = |tree: &WidgetTree, point: Offset| {
        tree.hit_test(point)
            .and_then(|render| tree.element_for_render(render))
    };
    // Points along the whole strip — including near its main-axis edges
    // and off the centerline — resolve to the divider, not the root.
    for point in [
        Offset::new(10.5, 5.),
        Offset::new(17.5, 95.),
        Offset::new(12., 49.),
        Offset::new(14., 0.5),
    ] {
        assert_ne!(
            hit_at(&tree, point),
            Some(root),
            "divider must be hittable at {point:?}"
        );
    }
    // Just outside the 8px strip — clear of the 10px panes — the root
    // background answers again.
    for point in [Offset::new(9.5, 10.), Offset::new(18.5, 90.)] {
        assert_eq!(hit_at(&tree, point), Some(root));
    }

    let mut tree = WidgetTree::new();
    let root = tree
        .mount(SplitView::vertical(box_(10., 10.), box_(10., 10.)).into())
        .expect("mount");
    tight(&mut tree, root, 100., 200.);
    for point in [
        Offset::new(5., 10.5),
        Offset::new(95., 17.5),
        Offset::new(50., 12.),
    ] {
        assert_ne!(
            hit_at(&tree, point),
            Some(root),
            "vertical divider must be hittable at {point:?}"
        );
    }
    // Clear of the centered 10px panes above and below the strip.
    for point in [Offset::new(5., 9.5), Offset::new(5., 18.5)] {
        assert_eq!(hit_at(&tree, point), Some(root));
    }
}

#[test]
fn split_view_constrained_cross_extent_bounds_divider() {
    // A 60px-tall parent bounds the cross fill: the divider is 8x60 while
    // the panes keep their intrinsic sizes, vertically centered. The tree
    // itself is loose so the fixed-size parent can actually resolve to 60:
    // a tight parent would win over the child, as with any ConstrainedBox.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::SizedBox::new()
                .width(200.)
                .height(60.)
                .child(SplitView::horizontal(box_(10., 10.), box_(10., 10.)))
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 100.)))
        .expect("layout");
    let row = only_child(&tree, root);
    let kids = tree.children(row).expect("row children").to_vec();
    let (divider_origin, divider_size) = bounds(&tree, kids[1]);
    let (first_origin, first_size) = bounds(&tree, kids[0]);
    assert_eq!(divider_origin, Offset::new(10., 0.));
    assert_eq!(divider_size, Size::new(8., 60.));
    assert_eq!(first_origin, Offset::new(0., 25.));
    assert_eq!(first_size, Size::new(10., 10.));
}

#[test]
fn split_view_unbounded_cross_degrades_without_collapse() {
    // With no bounded cross extent the divider shrinks to its child (empty)
    // instead of failing: enforcement has nothing to clamp to and the
    // inner alignment falls back to child size.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(SplitView::horizontal(box_(10., 10.), box_(10., 10.)).into())
        .expect("mount");
    tree.layout(Constraints::unbounded()).expect("layout");
    let kids = tree.children(root).expect("row children").to_vec();
    let (_, divider_size) = bounds(&tree, kids[1]);
    assert_eq!(divider_size, Size::new(8., 0.));
}

#[test]
fn split_view_pan_reports_main_axis_delta() {
    let deltas = Rc::new(RefCell::new(Vec::new()));
    let moved = deltas.clone();
    let mut tree = WidgetTree::new();
    tree.mount(
        SplitView::horizontal(box_(10., 10.), box_(10., 10.))
            .on_split_changed(move |delta| moved.borrow_mut().push(delta))
            .into(),
    )
    .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 100.)))
        .expect("layout");
    let now = Instant::now();
    let down = PointerEvent {
        pointer: 5,
        position: Offset::new(12., 70.),
        phase: PointerPhase::Down,
        time: now,
    };
    assert!(
        tree.dispatch_device_gesture_in_window(9, 11, down)
            .is_some()
    );
    let mv = PointerEvent {
        pointer: 5,
        position: Offset::new(32., 70.),
        phase: PointerPhase::Move,
        time: now,
    };
    assert!(tree.dispatch_device_gesture_in_window(9, 11, mv).is_some());
    let up = PointerEvent {
        phase: PointerPhase::Up,
        position: Offset::new(32., 70.),
        ..down
    };
    let _ = tree.dispatch_device_gesture_in_window(9, 11, up);
    assert_eq!(*deltas.borrow(), vec![20.]);
}

fn pointer_at(x: f32, phase: PointerPhase) -> PointerEvent {
    PointerEvent {
        pointer: 5,
        position: Offset::new(x, 70.),
        phase,
        time: Instant::now(),
    }
}

#[test]
fn split_view_callback_replacement_applies_to_new_gestures() {
    let first = Rc::new(RefCell::new(Vec::new()));
    let second = Rc::new(RefCell::new(Vec::new()));
    let first_moved = first.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .on_split_changed(move |delta| first_moved.borrow_mut().push(delta))
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let drag = |tree: &mut WidgetTree| {
        assert!(
            tree.dispatch_device_gesture_in_window(9, 11, pointer_at(12., PointerPhase::Down))
                .is_some()
        );
        assert!(
            tree.dispatch_device_gesture_in_window(9, 11, pointer_at(32., PointerPhase::Move))
                .is_some()
        );
        let _ = tree.dispatch_device_gesture_in_window(9, 11, pointer_at(32., PointerPhase::Up));
    };
    drag(&mut tree);
    assert_eq!(*first.borrow(), vec![20.]);

    // A replacement callback observes gestures started after the update;
    // the old one stays isolated. (An in-flight stream keeps the
    // recognizer cloned at down-time; the lifecycle is unchanged.)
    let second_moved = second.clone();
    tree.update(
        root,
        SplitView::horizontal(box_(10., 10.), box_(10., 10.))
            .on_split_changed(move |delta| second_moved.borrow_mut().push(delta))
            .into(),
    )
    .expect("update");
    tight(&mut tree, root, 200., 100.);
    drag(&mut tree);
    assert_eq!(*first.borrow(), vec![20.]);
    assert_eq!(*second.borrow(), vec![20.]);
}

#[test]
fn split_view_mounted_fraction_change_moves_divider() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(500., 10.), box_(500., 10.))
                .split_fraction(0.25)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 208., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    assert_eq!(bounds(&tree, kids[1]).0.x, 50.);

    tree.update(
        root,
        SplitView::horizontal(box_(500., 10.), box_(500., 10.))
            .split_fraction(0.75)
            .into(),
    )
    .expect("update");
    tight(&mut tree, root, 208., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    assert_eq!(bounds(&tree, kids[1]).0.x, 150.);
}

#[test]
fn split_view_removal_during_drag_delivers_nothing() {
    let deltas = Rc::new(RefCell::new(Vec::new()));
    let moved = deltas.clone();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Container::with_child(
                SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                    .on_split_changed(move |delta| moved.borrow_mut().push(delta)),
            )
            .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    assert!(
        tree.dispatch_device_gesture_in_window(9, 11, pointer_at(12., PointerPhase::Down))
            .is_some()
    );
    // Unmount the divider mid-drag through a same-type parent update.
    tree.update(
        root,
        incular_widgets::Container::with_child(box_(50., 50.)).into(),
    )
    .expect("update");
    tight(&mut tree, root, 200., 100.);
    assert_eq!(
        tree.dispatch_device_gesture_in_window(9, 11, pointer_at(32., PointerPhase::Move)),
        None
    );
    assert_eq!(
        tree.dispatch_device_gesture_in_window(9, 11, pointer_at(32., PointerPhase::Up)),
        None
    );
    assert!(deltas.borrow().is_empty());
}

#[test]
fn split_view_colored_divider_renders_full_height_bar() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .divider_color(Color::rgba(255, 0, 0, 255))
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    // Walk the single-child chain (detector, hit box, align, visual) to
    // the painted bar; every link down to the bar spans the full height.
    let mut chain = vec![kids[1]];
    while let Some(&last) = chain.last() {
        let next = tree.children(last).expect("divider chain").to_vec();
        if next.len() != 1 {
            break;
        }
        chain.push(next[0]);
    }
    let bar = chain
        .iter()
        .copied()
        .find(|node| bounds(&tree, *node).1.width == 1.)
        .expect("1px visual bar");
    let bar_index = chain.iter().position(|node| *node == bar).expect("index");
    for node in &chain[..=bar_index] {
        assert_eq!(bounds(&tree, *node).1.height, 100.);
    }
    // Default 1px visual bar centered in the 8px hit strip, full height.
    assert_eq!(bounds(&tree, bar).0, Offset::new(13.5, 0.));
    assert_eq!(bounds(&tree, bar).1, Size::new(1., 100.));
}

#[test]
fn split_view_replacement_updates_geometry() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SplitView::horizontal(box_(10., 10.), box_(10., 10.))
                .first_extent(40.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (divider_origin, _) = bounds(&tree, kids[1]);
    assert_eq!(divider_origin.x, 40.);

    tree.update(
        root,
        SplitView::horizontal(box_(10., 10.), box_(10., 10.))
            .split_position(SplitPosition::FromStart(90.))
            .into(),
    )
    .expect("update");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("row children").to_vec();
    let (moved_origin, _) = bounds(&tree, kids[1]);
    assert_eq!(moved_origin.x, 90.);
}

#[test]
fn overflow_bar_builder_defaults_match_new() {
    let children = vec![box_(10., 10.)];
    let built = OverflowBar::builder().children(children.clone()).build();
    assert_eq!(built, OverflowBar::new(children));
}

#[test]
fn overflow_bar_single_row_applies_spacing() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            OverflowBar::new([box_(30., 10.), box_(30., 10.)])
                .spacing(5.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 100.);
    let kids = tree.children(root).expect("bar children").to_vec();
    assert_eq!(kids.len(), 2);
    let (first_origin, _) = bounds(&tree, kids[0]);
    let (second_origin, _) = bounds(&tree, kids[1]);
    assert_eq!(first_origin, Offset::new(0., 0.));
    assert_eq!(second_origin, Offset::new(35., 0.));
}

#[test]
fn overflow_bar_wraps_with_overflow_spacing() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            OverflowBar::new([box_(30., 10.), box_(30., 10.)])
                .spacing(5.)
                .overflow_spacing(7.)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 40., 200.);
    let kids = tree.children(root).expect("bar children").to_vec();
    assert_eq!(kids.len(), 2);
    let (first_origin, _) = bounds(&tree, kids[0]);
    let (second_origin, _) = bounds(&tree, kids[1]);
    assert_eq!(first_origin, Offset::new(0., 0.));
    assert_eq!(second_origin, Offset::new(0., 17.));
}

#[test]
fn overflow_bar_cross_alignment_centers_run() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            OverflowBar::new([box_(30., 10.), box_(30., 20.)])
                .spacing(5.)
                .overflow_alignment(WrapCrossAlignment::Center)
                .into(),
        )
        .expect("mount");
    tight(&mut tree, root, 200., 200.);
    let kids = tree.children(root).expect("bar children").to_vec();
    let (first_origin, _) = bounds(&tree, kids[0]);
    let (second_origin, _) = bounds(&tree, kids[1]);
    assert_eq!(second_origin, Offset::new(35., 0.));
    assert_eq!(first_origin, Offset::new(0., 5.));
}
