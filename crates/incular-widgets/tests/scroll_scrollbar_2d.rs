//! Scrollbar and two-dimensional scrolling coverage.
//!
//! Fifth scrolling group: the retained overlay scrollbar (`scrollbar_*`
//! geometry shared by paint and input in `incular-scroll`, driven through
//! `WidgetTree::scrollbar_pointer`), the standalone `RawScrollbar` value
//! (single `geometry()` owner for paint and input), and the
//! two-dimensional scroll engine. Wheel, draggable sheets, and physics
//! internals belong to later groups; the existing `advanced_scrolling`
//! suite covers their basics and is referenced, not duplicated.

mod common;

use common::*;
use incular_config::{Axis, AxisDirection, Constraints};
use incular_core::{Color, Offset, PointerPhase, Size};
use incular_rendering::PaintCommand;
use incular_scroll::MetricOwner;
use incular_widgets::{
    Column, DiagonalDragBehavior, RawScrollbar, RawScrollbarOrientation, RawScrollbarStyle,
    TwoDimensionalChildDelegate, TwoDimensionalConstraints, TwoDimensionalScrollView,
    TwoDimensionalScrollable, TwoDimensionalViewport, internal::TreeError,
};
use std::time::Instant;

fn rows(count: usize) -> Vec<Widget> {
    (0..count)
        .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
        .collect()
}

fn mount_tight(
    tree: &mut WidgetTree,
    widget: Widget,
    w: f32,
    h: f32,
) -> incular_widgets::internal::ElementId {
    let root = tree.mount(widget).expect("mount");
    tree.layout(Constraints::tight(Size::new(w, h)))
        .expect("layout");
    root
}

fn thumb_rects(tree: &mut WidgetTree) -> Vec<incular_core::Rect> {
    tree.paint()
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::RRect { rrect, .. } => Some(rrect.rect),
            _ => None,
        })
        .collect()
}

fn grid_view() -> TwoDimensionalScrollView<incular_widgets::ChildVicinity> {
    TwoDimensionalScrollView::new(TwoDimensionalChildDelegate::new(10, 12, Some), 20., 30.)
}

#[test]
fn overlay_scrollbar_thumb_tracks_extents() {
    // Five 40px rows in a 100px viewport scroll 100px; the painted thumb
    // is viewport/content of the track and sits at the offset fraction.
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        incular_widgets::ListView::new(rows(5))
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(controller.max_offset(), 100.);
    let thumbs = thumb_rects(&mut tree);
    assert_eq!(thumbs.len(), 1);
    // 100/200 of the 100px track, floored by the 24px minimum.
    assert_eq!(thumbs[0].size.height, 50.);
    assert_eq!(thumbs[0].origin.y, 0.);

    assert!(controller.jump_to(50.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    let thumbs = thumb_rects(&mut tree);
    assert_eq!(thumbs.len(), 1);
    // Half the 50px travel at half offset.
    assert_eq!(thumbs[0].origin.y, 25.);
    assert_eq!(thumbs[0].size.height, 50.);

    // Zero scrollable range hides the overlay: content fits, so only the
    // (transparent, unpainted) track would remain.
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        incular_widgets::ListView::new(rows(2)).into(),
        100.,
        100.,
    );
    tree.update_compositor(Instant::now()).expect("compositor");
    assert!(thumb_rects(&mut tree).is_empty());
}

#[test]
fn overlay_scrollbar_style_toggles_paint() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        incular_widgets::ListView::new(rows(5))
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    tree.update_compositor(Instant::now()).expect("compositor");
    // Default style: transparent track, one thumb rrect.
    assert_eq!(thumb_rects(&mut tree).len(), 1);

    // An opaque track paints both rects through the same geometry.
    // Paint-only controller mutations apply on the next viewport
    // repaint, which a scroll-driven layout provides (the revision bump
    // is for reactive observation, not push invalidation).
    controller.set_scrollbar_style(incular_scroll::ScrollbarStyle {
        track_color: Color::rgba(0, 0, 0, 255),
        ..incular_scroll::ScrollbarStyle::default()
    });
    assert!(controller.jump_to(10.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    let rects = thumb_rects(&mut tree);
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].size, Size::new(8., 100.));

    // Hover-reveal hides the whole overlay — track included — until
    // hovered, while content still scrolls programmatically.
    controller.set_scrollbar_thumb_visibility(false);
    assert!(controller.jump_to(40.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(thumb_rects(&mut tree).len(), 0);
    assert_eq!(controller.offset(), 40.);
    // The viewport still owns hit testing: hovering the track reveals
    // both rects on the repaint the hover itself schedules.
    assert!(tree.scrollbar_pointer(PointerPhase::Move, Offset::new(96., 50.)));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    assert_eq!(thumb_rects(&mut tree).len(), 2);
}

#[test]
fn overlay_scrollbar_drag_and_track_share_geometry() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        incular_widgets::ListView::new(rows(5))
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    // The thumb occupies the top 50px of the right-edge track.
    assert!(tree.scrollbar_pointer(PointerPhase::Down, Offset::new(96., 10.)));
    assert!(tree.scrollbar_pointer(PointerPhase::Move, Offset::new(96., 60.)));
    assert_eq!(controller.offset(), 100.);
    assert!(tree.scrollbar_pointer(PointerPhase::Up, Offset::new(96., 60.)));

    // A track click below the thumb pages one viewport toward it; a click
    // above pages back. Both derive from the same painted geometry.
    assert!(controller.jump_to(0.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(tree.scrollbar_pointer(PointerPhase::Down, Offset::new(96., 90.)));
    assert_eq!(controller.offset(), 100.);
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(tree.scrollbar_pointer(PointerPhase::Down, Offset::new(96., 10.)));
    assert_eq!(controller.offset(), 0.);
}

#[test]
fn overlay_scrollbar_cached_paint_updates_hit_geometry() {
    let controller = ScrollController::new();
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        incular_widgets::ListView::new(rows(5))
            .controller(controller.clone())
            .into(),
        100.,
        100.,
    );
    tree.update_compositor(Instant::now()).expect("compositor");
    let _ = tree.paint();
    let before = tree.diagnostics();

    assert!(controller.jump_to(50.));
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    tree.update_compositor(Instant::now()).expect("compositor");
    // The thumb repaints at the new offset while retained pictures
    // replay, and the drag path consumes the updated geometry: grabbing
    // the moved thumb and dragging to the track end reaches the max.
    let thumbs = thumb_rects(&mut tree);
    assert_eq!(thumbs.len(), 1);
    assert_eq!(thumbs[0].origin.y, 25.);
    let after = tree.diagnostics();
    assert!(after.display_lists_reused > before.display_lists_reused);
    let grab = Offset::new(96., 30.);
    assert!(tree.scrollbar_pointer(PointerPhase::Down, grab));
    assert!(tree.scrollbar_pointer(PointerPhase::Move, Offset::new(96., 95.)));
    assert_eq!(controller.offset(), 100.);
    assert!(tree.scrollbar_pointer(PointerPhase::Up, Offset::new(96., 95.)));
    let _ = root;
}

#[test]
fn raw_scrollbar_geometry_covers_orientations() {
    for (orientation, track_origin, track_size, thumb_size) in [
        (
            RawScrollbarOrientation::Right,
            Offset::new(112., 0.),
            Size::new(8., 100.),
            Size::new(8., 24.),
        ),
        (
            RawScrollbarOrientation::Left,
            Offset::new(0., 0.),
            Size::new(8., 100.),
            Size::new(8., 24.),
        ),
        (
            RawScrollbarOrientation::Bottom,
            Offset::new(0., 92.),
            Size::new(120., 8.),
            Size::new(24., 8.),
        ),
        (
            RawScrollbarOrientation::Top,
            Offset::new(0., 0.),
            Size::new(120., 8.),
            Size::new(24., 8.),
        ),
    ] {
        let controller = ScrollController::new();
        controller.set_metrics_context(Axis::Vertical, false);
        controller
            .update_extents(500., 100.)
            .expect("free controller publishes");
        let mut bar = RawScrollbar::new(controller);
        bar.set_style(RawScrollbarStyle {
            orientation,
            ..RawScrollbarStyle::default()
        });
        // 100/500 of the 100px track, floored by the 24px minimum.
        let geometry = bar.geometry(Size::new(120., 100.));
        assert!(geometry.visible);
        assert_eq!(geometry.track.origin, track_origin);
        assert_eq!(geometry.track.size, track_size);
        assert_eq!(geometry.thumb.size, thumb_size);
    }
}

#[test]
fn raw_scrollbar_reversed_and_empty_extents() {
    // A reversed metrics context mirrors the thumb: offset zero sits at
    // the track end.
    let controller = ScrollController::new();
    controller.set_metrics_context(Axis::Vertical, true);
    controller
        .update_extents(500., 100.)
        .expect("free controller publishes");
    controller.jump_to(0.);
    let bar = RawScrollbar::new(controller);
    let geometry = bar.geometry(Size::new(120., 100.));
    assert!(geometry.visible);
    assert_eq!(geometry.thumb.origin.y, 76.);

    // Nothing scrollable means nothing painted and no input accepted.
    let controller = ScrollController::new();
    controller.set_metrics_context(Axis::Vertical, false);
    controller
        .update_extents(0., 0.)
        .expect("free controller publishes");
    let mut bar = RawScrollbar::new(controller);
    assert!(!bar.geometry(Size::new(120., 100.)).visible);
    assert!(!bar.pointer_down(Size::new(120., 100.), Offset::new(116., 50.)));
    assert!(!bar.pointer_move(Size::new(120., 100.), Offset::new(116., 60.)));
    assert!(!bar.hit_test(Size::new(120., 100.), Offset::new(116., 50.)));
}

#[test]
fn raw_scrollbar_drag_track_and_replacement() {
    let controller = ScrollController::new();
    controller.set_metrics_context(Axis::Vertical, false);
    controller
        .update_extents(500., 100.)
        .expect("free controller publishes");
    controller.jump_to(200.);
    let mut bar = RawScrollbar::new(controller.clone());
    let size = Size::new(120., 100.);
    let geometry = bar.geometry(size);
    // Dragging the painted thumb to the track end reaches the max through
    // the same geometry paint used: no second formula.
    let thumb_point = Offset::new(
        geometry.thumb.origin.x + geometry.thumb.size.width * 0.5,
        geometry.thumb.origin.y + geometry.thumb.size.height * 0.5,
    );
    assert!(bar.hit_test(size, thumb_point));
    assert!(bar.pointer_down(size, thumb_point));
    assert!(bar.is_dragging());
    assert!(bar.pointer_move(size, Offset::new(116., 88.)));
    assert_eq!(controller.offset(), controller.max_offset());
    bar.pointer_up();
    assert!(!bar.is_dragging());

    // A track click above the thumb pages one viewport back.
    assert!(controller.jump_to(200.));
    assert!(bar.pointer_down(size, Offset::new(116., 5.)));
    assert_eq!(controller.offset(), 100.);
    assert!(!bar.is_dragging());

    // Replacement is structural: a new scrollbar owns the new handle and
    // the old one keeps driving its own controller in isolation.
    let other = ScrollController::new();
    other.set_metrics_context(Axis::Vertical, false);
    other
        .update_extents(500., 100.)
        .expect("free controller publishes");
    let other_bar = RawScrollbar::new(other.clone());
    assert!(controller.jump_to(50.));
    assert_eq!(bar.controller().offset(), 50.);
    assert_eq!(other_bar.controller().offset(), 0.);
    // Restyling drops an active drag rather than retargeting it.
    assert!(bar.pointer_down(size, thumb_point));
    bar.set_style(RawScrollbarStyle::default());
    assert!(!bar.is_dragging());
}

#[test]
fn two_dimensional_layout_materializes_window() {
    let mut view = grid_view();
    let layout = view
        .viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    // Twelve 30px columns and ten 20px rows; 90x60 shows the leading
    // window plus the default cache.
    assert_eq!(layout.content_size, Size::new(360., 200.));
    assert_eq!(view.scrollable().horizontal_controller().max_offset(), 270.);
    assert_eq!(view.scrollable().vertical_controller().max_offset(), 140.);
    assert_eq!(
        layout.hit_test(Offset::new(15., 10.)),
        Some(incular_widgets::ChildVicinity::new(0, 0))
    );
    assert!(
        layout
            .children
            .iter()
            .all(|child| child.constraints.max_width == 30.)
    );
}

#[test]
fn two_dimensional_diagonal_and_independent_bounds() {
    let mut view = grid_view();
    // Extents establish on first layout; deltas before that consume
    // nothing.
    view.viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    view.scrollable_mut()
        .set_diagonal_drag_behavior(DiagonalDragBehavior::Free);
    // A diagonal delta applies to both axes at once.
    let delta = view.apply_delta(Offset::new(45., 35.));
    assert_eq!(delta.horizontal.consumed, 45.);
    assert_eq!(delta.vertical.consumed, 35.);
    let scrolled = view
        .viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    assert!(
        scrolled
            .children
            .iter()
            .any(|child| child.vicinity.x_index > 0)
    );
    assert!(
        scrolled
            .children
            .iter()
            .any(|child| child.vicinity.y_index > 0)
    );
    // Each axis clamps independently: overshooting horizontal leaves the
    // vertical position untouched.
    view.scrollable().horizontal_controller().jump_to(10_000.);
    assert_eq!(view.scrollable().horizontal_controller().offset(), 270.);
    assert_eq!(view.scrollable().vertical_controller().offset(), 35.);
}

#[test]
fn two_dimensional_diagonal_behaviors() {
    // Free applies both components; the locking default keeps the
    // dominant axis (ties break to the main axis); weighted attenuates
    // the minor component.
    for (behavior, main, input, horizontal, vertical) in [
        (
            DiagonalDragBehavior::Free,
            Axis::Vertical,
            Offset::new(45., 35.),
            45.,
            35.,
        ),
        // Locking keeps the dominant axis; ties break to the main axis.
        (
            DiagonalDragBehavior::None,
            Axis::Vertical,
            Offset::new(45., 35.),
            45.,
            0.,
        ),
        (
            DiagonalDragBehavior::None,
            Axis::Horizontal,
            Offset::new(35., 35.),
            35.,
            0.,
        ),
        (
            DiagonalDragBehavior::None,
            Axis::Vertical,
            Offset::new(35., 45.),
            0.,
            45.,
        ),
        (
            DiagonalDragBehavior::Weighted,
            Axis::Vertical,
            Offset::new(45., 35.),
            35.15625,
            25.15625,
        ),
    ] {
        let mut view = grid_view();
        view.scrollable_mut().set_diagonal_drag_behavior(behavior);
        view.scrollable_mut().set_main_axis(main);
        view.viewport_mut()
            .layout(Size::new(90., 60.))
            .expect("free controller publishes");
        let delta = view.apply_delta(input);
        assert!((delta.horizontal.consumed - horizontal).abs() < 0.01);
        assert!((delta.vertical.consumed - vertical).abs() < 0.01);
    }
}

#[test]
fn two_dimensional_reversed_axes_anchor_end() {
    let mut view = grid_view();
    // Establish extents first: jumps before layout clamp to zero.
    view.viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    view.scrollable_mut()
        .set_axis_directions(AxisDirection::Left, AxisDirection::Up);
    assert_eq!(
        view.scrollable().axis_directions(),
        (AxisDirection::Left, AxisDirection::Up)
    );
    view.scrollable().horizontal_controller().jump_to(270.);
    view.scrollable().vertical_controller().jump_to(140.);
    let layout = view
        .viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    // Trailing anchors: the last column and row paint at the origin.
    let first = layout
        .children
        .iter()
        .find(|child| child.visible)
        .expect("visible reversed child");
    assert_eq!(first.vicinity, incular_widgets::ChildVicinity::new(9, 7));
    assert_eq!(first.paint_offset, Offset::new(0., 0.));
}

#[test]
fn two_dimensional_cache_resize_and_measure() {
    let mut view = grid_view();
    view.viewport_mut()
        .set_cache_extent(0., incular_widgets::CacheExtentStyle::Pixels);
    let uncached = view
        .viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    view.viewport_mut()
        .set_cache_extent(0.5, incular_widgets::CacheExtentStyle::Viewport);
    let cached = view
        .viewport_mut()
        .layout(Size::new(90., 60.))
        .expect("free controller publishes");
    // Half a viewport of cache on each side materializes more rows.
    assert!(cached.row_range.len() > uncached.row_range.len());

    // Resizing the viewport widens both ranges and shrinks both maxima.
    let resized = view
        .viewport_mut()
        .layout(Size::new(180., 120.))
        .expect("free controller publishes");
    assert_eq!(view.scrollable().horizontal_controller().max_offset(), 180.);
    assert_eq!(view.scrollable().vertical_controller().max_offset(), 80.);
    assert!(resized.row_range.len() > cached.row_range.len());

    // Measured extents feed back through revisions and shrink content.
    let row_revision = view.viewport().row_revision();
    let measured = view
        .viewport_mut()
        .layout_with_measure(Size::new(90., 60.), |vicinity, constraints| {
            Size::new(
                (10.0 + vicinity.x_index as f32).min(constraints.max_width),
                (8.0 + vicinity.y_index as f32).min(constraints.max_height),
            )
        })
        .expect("free controller publishes");
    assert!(view.viewport().row_revision() > row_revision);
    assert!(measured.content_size.width < 360.);
    assert!(measured.content_size.height < 200.);
}

#[test]
fn two_dimensional_axis_sharing_rejected_with_pair_intact() {
    // Each independent axis has clearly defined ownership: two retained
    // 2D viewports sharing the horizontal controller fail on the second
    // viewport — naming the first viewport's element — while the first
    // viewport's pair stays exactly intact on both axes. The never-laid-out
    // second vertical controller stays free.
    let horizontal = ScrollController::new();
    let first_vertical = ScrollController::new();
    let second_vertical = ScrollController::new();
    let view = |vertical: ScrollController| -> Widget {
        let viewport: Widget = TwoDimensionalViewport::new(
            TwoDimensionalChildDelegate::new(10, 12, |_| {
                Some(Widget::box_(Size::new(30., 20.), Color::WHITE))
            }),
            horizontal.clone(),
            vertical,
            20.,
            30.,
        )
        .into();
        incular_widgets::SizedBox::from_dimensions(Some(100.), Some(100.), Some(viewport)).into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new(vec![
                view(first_vertical.clone()),
                view(second_vertical.clone()),
            ])
            .into(),
        )
        .expect("mount defers attachment");
    let error = tree
        .layout(Constraints::tight(Size::new(100., 250.)))
        .unwrap_err();
    let kids = tree.children(root).expect("viewports").to_vec();
    assert_eq!(kids.len(), 2);
    let viewport_of =
        |sized: incular_widgets::internal::ElementId| tree.children(sized).expect("viewport")[0];
    match error {
        TreeError::DuplicateScrollAttachment {
            owner_tree,
            owner,
            attempted,
        } => {
            assert_eq!(owner_tree, tree.tree_id());
            assert_eq!(owner, Some(viewport_of(kids[0])));
            assert_eq!(attempted, viewport_of(kids[1]));
        }
        other => panic!("unexpected failure: {other:?}"),
    }
    // First pair intact on both axes; the second vertical never published.
    assert_eq!(horizontal.content_extent(), 360.);
    assert_eq!(horizontal.viewport_extent(), 100.);
    assert_eq!(horizontal.max_offset(), 260.);
    assert_eq!(first_vertical.content_extent(), 200.);
    assert_eq!(first_vertical.max_offset(), 100.);
    assert_eq!(second_vertical.max_offset(), 0.);
    assert_eq!(horizontal.metric_owner(), Some(tree.tree_id()));
    assert_eq!(first_vertical.metric_owner(), Some(tree.tree_id()));
    assert_eq!(second_vertical.metric_owner(), None);
}

#[test]
fn two_dimensional_pair_publication_is_atomic() {
    // The model helper validates both axes before writing either: with
    // the horizontal axis owned elsewhere the call fails leaving both
    // axes — extents, offsets, revisions — exactly as they were. The
    // layout entry enforces the same rule before measuring anything.
    let horizontal = ScrollController::new();
    let vertical = ScrollController::new();
    let scrollable = TwoDimensionalScrollable::new(horizontal.clone(), vertical.clone());
    scrollable
        .update_extents(300., 100., 360., 200.)
        .expect("free pair publishes");
    assert!(horizontal.jump_to(20.));
    let horizontal_revision = horizontal.revision();
    let vertical_revision = vertical.revision();
    // Own the SECOND axis: without pair validation the horizontal write
    // would land before the vertical rejection — a partial publication.
    let lease = vertical
        .try_attach(MetricOwner::of_tree(7))
        .expect("free axis attaches");
    let error = scrollable.update_extents(50., 50., 60., 60.).unwrap_err();
    assert_eq!(error.owner_tree(), Some(7));
    assert_eq!(horizontal.content_extent(), 300.);
    assert_eq!(horizontal.viewport_extent(), 100.);
    assert_eq!(horizontal.offset(), 20.);
    assert_eq!(horizontal.revision(), horizontal_revision);
    assert_eq!(vertical.content_extent(), 360.);
    assert_eq!(vertical.viewport_extent(), 200.);
    assert_eq!(vertical.revision(), vertical_revision);
    assert!(lease.release());
    scrollable
        .update_extents(50., 50., 60., 60.)
        .expect("freed pair publishes");
    assert_eq!(horizontal.content_extent(), 50.);
    assert_eq!(vertical.content_extent(), 60.);
    // Same rule through the layout entry: an owned axis fails before any
    // measurement or child work.
    let mut viewport = TwoDimensionalViewport::new(
        TwoDimensionalChildDelegate::new(10, 12, Some),
        horizontal.clone(),
        vertical.clone(),
        20.,
        30.,
    );
    let owned = horizontal
        .try_attach(MetricOwner::of_tree(9))
        .expect("free axis attaches");
    let error = match viewport.layout(Size::new(90., 60.)) {
        Ok(_) => panic!("owned axis refuses 2D layout"),
        Err(error) => error,
    };
    assert_eq!(error.owner_tree(), Some(9));
    assert_eq!(horizontal.content_extent(), 50.);
    assert_eq!(vertical.content_extent(), 60.);
    assert!(owned.release());
    viewport
        .layout(Size::new(90., 60.))
        .expect("freed axes lay out");
    assert_eq!(horizontal.content_extent(), 360.);
}

#[test]
fn two_dimensional_axis_replacement_reuses_the_unchanged_axis() {
    // Swapping only the vertical controller acquires just the newcomer:
    // the horizontal lease keeps its identity, geometry, and open
    // activity; the replaced-away vertical tenure ends; the new pair
    // drives with fresh geometry.
    let horizontal = ScrollController::new();
    let first_vertical = ScrollController::new();
    let second_vertical = ScrollController::new();
    let horizontal_ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let vertical_ends = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted_horizontal = horizontal_ends.clone();
    let counted_vertical = vertical_ends.clone();
    let _horizontal_subscription = horizontal.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted_horizontal.set(counted_horizontal.get() + 1);
        }
        false
    });
    let _vertical_subscription = first_vertical.add_listener(move |notification| {
        if notification.kind == incular_scroll::ScrollNotificationType::End {
            counted_vertical.set(counted_vertical.get() + 1);
        }
        false
    });
    let delegate = || {
        TwoDimensionalChildDelegate::new(10, 12, |_| {
            Some(Widget::box_(Size::new(30., 20.), Color::WHITE))
        })
    };
    let view = |vertical: ScrollController| -> Widget {
        let viewport: Widget =
            TwoDimensionalViewport::new(delegate(), horizontal.clone(), vertical, 20., 30.).into();
        incular_widgets::SizedBox::from_dimensions(Some(100.), Some(100.), Some(viewport)).into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(view(first_vertical.clone()))
        .expect("mount defers attachment");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    assert!(horizontal.begin_activity());
    assert!(first_vertical.begin_activity());
    let horizontal_attachment = horizontal.attachment_id().expect("H owned");
    assert_eq!(horizontal.max_offset(), 260.);
    assert_eq!(first_vertical.max_offset(), 100.);
    tree.update(root, view(second_vertical.clone()))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("layout");
    // Unchanged axis untouched: same lease, same geometry, still open,
    // no End delivered.
    assert_eq!(horizontal.attachment_id(), Some(horizontal_attachment));
    assert_eq!(horizontal.max_offset(), 260.);
    assert!(!horizontal.begin_activity());
    assert_eq!(horizontal_ends.get(), 0);
    // Replaced-away vertical tenure ended; the newcomer drives fresh.
    assert_eq!(first_vertical.metric_owner(), None);
    assert_eq!(vertical_ends.get(), 1);
    assert_eq!(first_vertical.max_offset(), 100.);
    assert_eq!(second_vertical.metric_owner(), Some(tree.tree_id()));
    assert_eq!(second_vertical.max_offset(), 100.);
    assert!(horizontal.end_activity());
    assert!(
        !first_vertical.end_activity(),
        "transfer already closed the replaced-away tenure"
    );
}

#[test]
fn two_dimensional_delegate_and_constraints() {
    // from_rows derives counts from the grid shape.
    let delegate =
        incular_widgets::TwoDimensionalChildDelegate::from_rows(vec![vec![1, 2], vec![3, 4]]);
    assert_eq!(delegate.row_count(), 2);
    assert_eq!(delegate.column_count(), 2);
    assert_eq!(
        incular_widgets::TwoDimensionalChildDelegate::new(10, 12, Some).row_count(),
        10
    );
    // Negative constraint extents floor at zero.
    let constraints = TwoDimensionalConstraints::new(-5., 10.);
    assert_eq!(constraints.biggest(), Size::new(0., 10.));
    assert_eq!(
        TwoDimensionalConstraints::new(30., 20.).constrain(Size::new(100., 100.)),
        Size::new(30., 20.)
    );
}
