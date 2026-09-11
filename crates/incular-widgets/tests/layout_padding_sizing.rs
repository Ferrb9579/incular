//! Padding, alignment, and sizing-constraint contracts.
//!
//! Size and ratio validation floors at the descriptor (negative
//! factors to zero, aspect ratios to epsilon, box dimensions at
//! conversion); offsets stay meaningful, including negative padding
//! which expands. Layout math lives in `incular-layout`; widgets
//! resolve configuration and measure children.

use incular_config::{Alignment, Axis, Constraints, EdgeInsets};
use incular_core::{Color, Size};
use incular_rendering::PaintCommand;
use incular_text::TextStyle;
use incular_widgets::{
    Align, AspectRatio, Baseline, BoxFit, Center, ConstrainedBox, ConstraintsTransformBox,
    FittedBox, FractionallySizedBox, IntrinsicHeight, IntrinsicWidth, LimitedBox, OverflowBox,
    Padding, SizedBox, SizedOverflowBox, Text, UnconstrainedBox, Widget,
    internal::{ElementId, WidgetTree},
};

fn layout_tight(tree: &mut WidgetTree, width: f32, height: f32) {
    tree.layout(Constraints::tight(Size::new(width, height)))
        .expect("layout");
}

fn bounds_of(tree: &WidgetTree, id: ElementId) -> (f32, f32, f32, f32) {
    let bounds = tree.element_bounds(id).expect("bounds");
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

fn only_child(tree: &WidgetTree, id: ElementId) -> ElementId {
    tree.children(id).expect("child")[0]
}

fn white_box(width: f32, height: f32) -> Widget {
    Widget::box_(Size::new(width, height), Color::WHITE)
}

#[test]
fn padding_insets_and_insufficient_space() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Padding::all(10., white_box(30., 20.)).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 50., 40.));
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).0, 10.);

    // Padding larger than the constraints deflates the child to zero
    // instead of panicking or going negative.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Padding::all(15., white_box(30., 20.)).into())
        .expect("mount");
    layout_tight(&mut tree, 20., 20.);
    assert_eq!(bounds_of(&tree, root).2, 20.);
    let child = only_child(&tree, root);
    assert_eq!(bounds_of(&tree, child).0, 15.);
    assert_eq!(bounds_of(&tree, child).2, 0.);
}

#[test]
fn padding_negative_expands() {
    // Negative insets are meaningful offsets, not invalid sizes: they
    // grow the child constraints and shift the child outward.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Padding::all(-5., white_box(30., 20.)).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 20., 10.));
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).0, -5.);
}

#[test]
fn padding_constructors_agree() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(Padding::new(EdgeInsets::all(4.), child())),
        Widget::from(Padding::all(4., child()))
    );
    let built = Padding::builder()
        .padding(EdgeInsets::symmetric(4., 6.))
        .child(child())
        .build();
    assert_eq!(
        Widget::from(Padding::symmetric(4., 6., child())),
        Widget::from(built)
    );
    assert_eq!(
        Widget::from(Padding::zero(child())),
        Widget::from(Padding::all(0., child()))
    );
}

#[test]
fn align_factors_and_negative_floor() {
    // Center without factors fills tight constraints and centers.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Align::new(Alignment::CENTER, white_box(40., 20.)).into())
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    assert_eq!(bounds_of(&tree, root), (0., 0., 200., 100.));
    assert_eq!(
        bounds_of(&tree, only_child(&tree, root)),
        (80., 40., 40., 20.)
    );

    // Width factor scales the box to the child multiple.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Align::new(Alignment::CENTER, white_box(40., 20.))
                .width_factor(2.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 80.);

    // Negative factors are invalid ratios, floored to zero rather than
    // kept: the box collapses on that axis.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Align::new(Alignment::CENTER, white_box(40., 20.))
                .width_factor(-2.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 100.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 0.);
}

#[test]
fn center_delegates_to_align() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(Center::new(child())),
        Widget::from(Align::new(Alignment::CENTER, child()))
    );
    assert_eq!(
        Widget::from(Center::new(child()).width_factor(2.)),
        Widget::from(Align::new(Alignment::CENTER, child()).width_factor(2.))
    );
    let built = Center::builder().child(child()).build();
    assert_eq!(Widget::from(Center::new(child())), Widget::from(built));
}

#[test]
fn fractionally_sized_box_scales_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FractionallySizedBox::new(white_box(300., 20.))
                .width_factor(0.5)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).2, 100.);

    // Negative factors floor to zero.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FractionallySizedBox::new(white_box(300., 20.))
                .width_factor(-0.5)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).2, 0.);
}

#[test]
fn aspect_ratio_drives_size_and_floors_invalid() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(AspectRatio::new(2., white_box(10., 10.)).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 200., 100.));

    // Zero and negative ratios floor to epsilon instead of dividing by
    // zero: height resolves from the bounded width, collapsing the
    // width to epsilon while staying finite.
    for ratio in [0., -3.] {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(AspectRatio::new(ratio, white_box(10., 10.)).into())
            .expect("mount");
        tree.layout(Constraints::loose(Size::new(200., 200.)))
            .expect("layout");
        let (_, _, width, height) = bounds_of(&tree, root);
        assert!(width.is_finite() && height.is_finite());
        assert!(width < 1.0 && height == 200., "got ({width}, {height})");
    }
    // Builder and constructor agree, including the floor.
    assert_eq!(
        Widget::from(AspectRatio::new(2., white_box(1., 1.))),
        Widget::from(
            AspectRatio::builder()
                .aspect_ratio(2.)
                .child(white_box(1., 1.))
                .build()
        )
    );
}

#[test]
fn baseline_offsets_follow_target_deltas() {
    let text = |size: f32| {
        Widget::from(Text::new("Ag").style(TextStyle {
            size,
            ..Default::default()
        }))
    };
    // Targets above the child baseline push it down one-to-one;
    // targets below clamp at zero instead of shifting upward.
    let build = |baseline: f32| Widget::from(Baseline::new(baseline, text(16.)));
    let mut tree = WidgetTree::new();
    let first = tree.mount(build(30.)).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let first_y = bounds_of(&tree, only_child(&tree, first)).1;
    let mut tree = WidgetTree::new();
    let second = tree.mount(build(50.)).expect("mount");
    layout_tight(&mut tree, 200., 100.);
    let second_y = bounds_of(&tree, only_child(&tree, second)).1;
    // Raising the target by 20 pushes the child down by exactly 20.
    assert_eq!(second_y - first_y, 20.);
    // Negative baselines clamp to zero like a zero baseline.
    assert_eq!(
        Widget::from(Baseline::new(-5., text(16.))),
        Widget::from(Baseline::new(0., text(16.)))
    );
}

#[test]
fn fitted_box_contain_centers() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            FittedBox::new(white_box(40., 30.))
                .fit(BoxFit::Contain)
                .alignment(Alignment::CENTER)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 100.);
    // Contain scales by the limiting axis (100/30) and centers.
    let (_, y, width, height) = bounds_of(&tree, only_child(&tree, root));
    assert!((width - 133.33).abs() < 0.5, "width {width}");
    assert_eq!(height, 100.);
    assert!((y - 0.).abs() < 0.5);
}

#[test]
fn sized_overflow_box_reports_size_and_overflows() {
    // A shrinking child would fit, so force real overflow with an
    // unconstrained child that keeps its intrinsic 100px.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            SizedOverflowBox::new(
                Size::new(50., 50.),
                UnconstrainedBox::new(white_box(100., 20.)),
            )
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 50., 50.));
    // The overflow element reports constrained size while the
    // grandchild paints past the edge: find its 100px white rect.
    let widest = tree
        .paint()
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, color } if *color == Color::WHITE => Some(*rect),
            _ => None,
        })
        .max_by(|left, right| left.size.width.total_cmp(&right.size.width))
        .expect("white rect");
    assert_eq!(
        (widest.origin.x, widest.size.width),
        (0., 100.),
        "grandchild paints past the 50px box"
    );
}

#[test]
fn constrained_limited_overflow_boxes() {
    // ConstrainedBox imposes tight bounds.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            ConstrainedBox::new(Constraints::tight(Size::new(50., 40.)), white_box(10., 10.))
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 50., 40.));

    // LimitedBox caps unbounded measurement but passes bounded through.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            LimitedBox::new(white_box(100., 100.))
                .max_width(60.)
                .max_height(60.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::unbounded()).expect("layout");
    assert_eq!(
        bounds_of(&tree, only_child(&tree, root)),
        (0., 0., 60., 60.)
    );
    // Bounded incoming constraints pass straight through to the child.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            LimitedBox::new(white_box(100., 100.))
                .max_width(60.)
                .max_height(60.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(
        bounds_of(&tree, only_child(&tree, root)),
        (0., 0., 100., 100.)
    );

    // OverflowBox minimums force growth while the box reports constrained size.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            OverflowBox::new(white_box(10., 10.))
                .min_width(100.)
                .min_height(80.)
                .into(),
        )
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    assert_eq!(
        bounds_of(&tree, only_child(&tree, root)),
        (0., 0., 100., 80.)
    );
}

#[test]
fn unconstrained_and_intrinsic_aliases_measure_content() {
    // Both intrinsic aliases resolve through UnconstrainedBox and agree
    // behaviorally under tight constraints.
    for make in [
        Widget::from(UnconstrainedBox::new(white_box(30., 20.))),
        Widget::from(IntrinsicWidth::new(white_box(30., 20.))),
        Widget::from(IntrinsicHeight::new(white_box(30., 20.))),
    ] {
        let mut tree = WidgetTree::new();
        let root = tree.mount(make).expect("mount");
        layout_tight(&mut tree, 200., 200.);
        assert_eq!(
            bounds_of(&tree, only_child(&tree, root)),
            (0., 0., 30., 20.)
        );
    }
}

#[test]
fn intrinsic_width_step_rounds_measured_width() {
    // A 37px child with step_width 16 rounds its intrinsic width up to
    // 48; its height is untouched.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IntrinsicWidth::new(white_box(37., 11.))
                .step_width(16.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(500., 500.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 48., 11.));
}

#[test]
fn intrinsic_step_ignores_invalid_values() {
    let child = || white_box(37., 11.);
    // Zero, negative, and non-finite steps leave the extent unchanged;
    // unbounded constraints do not change the derived size either.
    for bad in [0., -4., f32::NAN, f32::INFINITY] {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                IntrinsicWidth::new(child())
                    .step_width(bad)
                    .step_height(bad)
                    .into(),
            )
            .expect("mount");
        tree.layout(Constraints::unbounded()).expect("layout");
        assert_eq!(bounds_of(&tree, root), (0., 0., 37., 11.), "step {bad}");
    }
    // A child that is already a multiple is unchanged.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            IntrinsicWidth::new(white_box(32., 11.))
                .step_width(16.)
                .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(500., 500.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root).2, 32.);
}

#[test]
fn intrinsic_step_stays_coherent_under_bounded_and_unbounded_parents() {
    // A bounded parent clamps the stepped size through the normal
    // constrain path; an unbounded parent keeps it verbatim.
    let build = || IntrinsicWidth::new(white_box(37., 11.)).step_width(16.);
    let mut tree = WidgetTree::new();
    let root = tree.mount(build().into()).expect("mount");
    tree.layout(Constraints::tight(Size::new(40., 40.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 40., 40.));
    let mut tree = WidgetTree::new();
    let root = tree.mount(build().into()).expect("mount");
    tree.layout(Constraints::unbounded()).expect("layout");
    assert_eq!(bounds_of(&tree, root), (0., 0., 48., 11.));
}

#[test]
fn intrinsic_width_step_builders_match_fluent_construction() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(IntrinsicWidth::new(child()).step_width(8.).step_height(4.)),
        Widget::from(
            IntrinsicWidth::builder()
                .step_width(8.)
                .step_height(4.)
                .child(child())
                .build()
        )
    );
}

#[test]
fn constraint_transform_box_routes_clip_behavior() {
    use incular_config::Clip;
    use incular_core::Rect;
    use incular_rendering::PaintCommand;

    let clip_rects = |tree: &mut WidgetTree| -> Vec<Rect> {
        tree.paint()
            .commands()
            .iter()
            .filter_map(|command| match command {
                PaintCommand::PushClip { rect } => Some(*rect),
                _ => None,
            })
            .collect()
    };

    let build = |clip| {
        ConstraintsTransformBox::new(
            |incoming| Constraints::tight(Size::new(incoming.max_width() / 2., 40.)),
            white_box(10., 10.),
        )
        .clip_behavior(clip)
    };
    // Clip::None adds no layer.
    let mut tree = WidgetTree::new();
    tree.mount(build(Clip::None).into()).expect("mount");
    layout_tight(&mut tree, 200., 200.);
    assert!(clip_rects(&mut tree).is_empty(), "Clip::None adds no layer");

    // A clipping behavior attaches one clip layer over the transformed
    // child, not a second paint-time clip.
    let mut tree = WidgetTree::new();
    tree.mount(build(Clip::HardEdge).into()).expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let clips = clip_rects(&mut tree);
    assert_eq!(clips.len(), 1, "one clip layer, got {clips:?}");
    assert_eq!(clips[0].size, Size::new(200., 200.));
}

#[test]
fn fitted_box_all_fit_modes_project_independently() {
    // A 40x20 child in a 100x100 fixed box. `element_bounds` folds the
    // ancestor fit transform, so its world rect is the placement under
    // that fit. sx = 2.5, sy = 5.
    use incular_widgets::internal::ImageFit;
    type Placement = (f32, f32, f32, f32);
    let cases: &[(ImageFit, Placement)] = &[
        // Fill stretch: 100x100.
        (ImageFit::Fill, (0., 0., 100., 100.)),
        // Contain min(2.5, 5) = 2.5 -> 100x50, centered vertically.
        (ImageFit::Contain, (0., 25., 100., 50.)),
        // Cover max(2.5, 5) = 5 -> 200x100, centered horizontally.
        (ImageFit::Cover, (-50., 0., 200., 100.)),
        (ImageFit::FitWidth, (0., 25., 100., 50.)),
        (ImageFit::FitHeight, (-50., 0., 200., 100.)),
        // None never scales: 40x20 centered.
        (ImageFit::None, (30., 40., 40., 20.)),
        // ScaleDown never upscales: same as None for a small source.
        (ImageFit::ScaleDown, (30., 40., 40., 20.)),
    ];
    for (fit, expected) in cases {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                SizedBox::from_size(Size::new(100., 100.))
                    .child(
                        FittedBox::new(white_box(40., 20.))
                            .fit(*fit)
                            .alignment(Alignment::CENTER),
                    )
                    .into(),
            )
            .expect("mount");
        layout_tight(&mut tree, 100., 100.);
        // Descend until the deepest sole child (the fitted 40x20 box).
        let mut inner = root;
        while let Some(kids) = tree.children(inner) {
            if kids.is_empty() {
                break;
            }
            inner = kids[0];
        }
        let (x, y, w, h) = bounds_of(&tree, inner);
        assert!(
            (x - expected.0).abs() < 0.5
                && (y - expected.1).abs() < 0.5
                && (w - expected.2).abs() < 0.5
                && (h - expected.3).abs() < 0.5,
            "{fit:?}: got ({x}, {y}, {w}, {h}) want {expected:?}"
        );
    }
}

#[test]
#[should_panic(expected = "invalid constraints")]
fn constrained_box_negative_panics_loudly() {
    // Invalid sizes fail at construction instead of normalizing
    // silently: callers hear about them immediately. (PreferredSize
    // shares this path but is not publicly reachable.)
    let _ = ConstrainedBox::new(Constraints::new(-5., -5., 40., 40.), white_box(10., 10.));
}

#[test]
fn sized_box_constructors_agree_and_clamp() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(SizedBox::from_size(Size::new(20., 30.))),
        Widget::from(SizedBox::new().width(20.).height(30.))
    );
    assert_eq!(
        Widget::from(SizedBox::square(12.)),
        Widget::from(SizedBox::new().width(12.).height(12.))
    );
    assert_eq!(
        Widget::from(SizedBox::shrink()),
        Widget::from(SizedBox::new().width(0.).height(0.))
    );
    assert_eq!(
        Widget::from(SizedBox::empty()),
        Widget::from(SizedBox::new())
    );
    // Negative dimensions clamp to zero at conversion, like the
    // builder path, instead of panicking. Core Size::new would panic
    // first, so the clamping path takes plain setter input. Loose
    // parents let the zero bound show; tight parents would dominate
    // through enforced clamping like Flutter.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(SizedBox::new().width(-5.).height(10.).child(child()).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).2, 0.);
    let built = SizedBox::builder()
        .width(-5.)
        .height(10.)
        .child(child())
        .build();
    assert_eq!(
        Widget::from(SizedBox::new().width(-5.).height(10.).child(child())),
        Widget::from(built)
    );
}

#[test]
fn mounted_padding_update_and_identical_reapply() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Padding::all(10., white_box(30., 20.)).into())
        .expect("mount");
    layout_tight(&mut tree, 200., 200.);
    let child = only_child(&tree, root);
    assert_eq!(bounds_of(&tree, child).0, 10.);
    let before = tree.diagnostics();

    tree.update(root, Padding::all(20., white_box(30., 20.)).into())
        .expect("update padding");
    layout_tight(&mut tree, 200., 200.);
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).0, 20.);
    assert_eq!(tree.children(root).expect("children")[0], child);
    let after_update = tree.diagnostics();
    assert!(after_update.layouts > before.layouts);

    let padding = Widget::from(Padding::all(20., white_box(30., 20.)));
    tree.update(root, padding).expect("reapply");
    layout_tight(&mut tree, 200., 200.);
    // Fresh values never pointer-match, so no bailout is expected; the
    // layout result must still agree exactly.
    let _ = tree.paint();
    let after_reapply = tree.diagnostics();
    assert_eq!(after_reapply.layouts, after_update.layouts);
    assert_eq!(bounds_of(&tree, only_child(&tree, root)).0, 20.);
}

#[test]
fn alignment_family_builders_match_fluent_construction() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(Align::new(Alignment::BOTTOM_RIGHT, child()).width_factor(2.)),
        Widget::from(
            Align::builder()
                .alignment(Alignment::BOTTOM_RIGHT)
                .width_factor(2.)
                .child(child())
                .build()
        )
    );
    assert_eq!(
        Widget::from(Center::new(child()).height_factor(3.)),
        Widget::from(Center::builder().height_factor(3.).child(child()).build())
    );
}

#[test]
fn sizing_family_builders_match_fluent_construction() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(ConstrainedBox::new(
            Constraints::tight(Size::new(5., 5.)),
            child()
        )),
        Widget::from(
            ConstrainedBox::builder()
                .constraints(Constraints::tight(Size::new(5., 5.)))
                .child(child())
                .build()
        )
    );
    assert_eq!(
        Widget::from(LimitedBox::new(child()).max_width(7.)),
        Widget::from(LimitedBox::builder().max_width(7.).child(child()).build())
    );
    assert_eq!(
        Widget::from(OverflowBox::new(child()).min_width(7.)),
        Widget::from(OverflowBox::builder().min_width(7.).child(child()).build())
    );
    assert_eq!(
        Widget::from(UnconstrainedBox::new(child()).constrained_axis(Axis::Vertical)),
        Widget::from(
            UnconstrainedBox::builder()
                .constrained_axis(Axis::Vertical)
                .child(child())
                .build()
        )
    );
    assert_eq!(
        Widget::from(IntrinsicWidth::new(child()).step_width(3.)),
        Widget::from(
            IntrinsicWidth::builder()
                .step_width(3.)
                .child(child())
                .build()
        )
    );
    assert_eq!(
        Widget::from(IntrinsicHeight::new(child())),
        Widget::from(IntrinsicHeight::builder().child(child()).build())
    );
    assert_eq!(
        Widget::from(SizedOverflowBox::new(Size::new(9., 9.), child())),
        Widget::from(
            SizedOverflowBox::builder()
                .size(Size::new(9., 9.))
                .child(child())
                .build()
        )
    );
}

#[test]
fn baseline_aspect_fractional_builders_match_fluent_construction() {
    let child = || white_box(10., 10.);
    assert_eq!(
        Widget::from(AspectRatio::new(2., child())),
        Widget::from(
            AspectRatio::builder()
                .aspect_ratio(2.)
                .child(child())
                .build()
        )
    );
    assert_eq!(
        Widget::from(Baseline::new(8., child())),
        Widget::from(Baseline::builder().baseline(8.).child(child()).build())
    );
    assert_eq!(
        Widget::from(FittedBox::new(child()).fit(BoxFit::Cover)),
        Widget::from(
            FittedBox::builder()
                .fit(BoxFit::Cover)
                .child(child())
                .build()
        )
    );
    assert_eq!(
        Widget::from(FractionallySizedBox::new(child()).width_factor(0.5)),
        Widget::from(
            FractionallySizedBox::builder()
                .width_factor(0.5)
                .child(child())
                .build()
        )
    );
}

#[test]
fn constraints_transform_box_rewrites_child_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            ConstraintsTransformBox::new(
                |incoming| Constraints::tight(Size::new(incoming.max_width() / 2., 40.)),
                white_box(10., 10.),
            )
            .into(),
        )
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(
        bounds_of(&tree, only_child(&tree, root)),
        (0., 0., 100., 40.)
    );
}
