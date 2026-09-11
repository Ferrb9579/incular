//! Overlay-portal and tooltip lifecycle coverage.
//!
//! `OverlayPortal` owns anchored in-view surfaces through the existing
//! retained transient registry (`WidgetTree::transient_surfaces`); no
//! second registry is introduced and cleanup is asserted through that
//! registry, paint partitions, hit testing, and semantics — never inferred
//! from visual disappearance alone. `RawTooltip` drives its overlay
//! through `RawTooltipController` production paths.

use incular_config::{Alignment, Constraints, EdgeInsets};
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::PaintCommand;
use incular_semantics::SemanticRole;
use incular_widgets::{
    FocusNode, LayerLink, OverlayPortal, Padding, RawTooltip, RawTooltipController, Semantics,
    Text, TooltipTriggerMode, TransientDismissPolicy, TransientDismissReason, TransientPlacement,
    TransientPresentation, TransientRole, Widget,
    internal::{ElementId, WidgetTree},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

fn popup_box(w: f32, h: f32) -> Widget {
    Widget::box_(Size::new(w, h), Color::BLACK)
}

fn open_portal() -> Widget {
    OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
        .overlay_child(popup_box(30., 40.))
        .role(TransientRole::Menu)
        .show(true)
        .into()
}

fn mount_tight(tree: &mut WidgetTree, widget: Widget) -> ElementId {
    mount_sized(tree, widget, 200., 200.)
}

fn mount_sized(tree: &mut WidgetTree, widget: Widget, w: f32, h: f32) -> ElementId {
    let root = tree.mount(widget).expect("mount");
    tree.layout(Constraints::tight(Size::new(w, h)))
        .expect("layout");
    root
}

fn find_box(tree: &WidgetTree, id: ElementId, size: Size) -> Option<ElementId> {
    if tree
        .element_bounds(id)
        .is_some_and(|rect| rect.size == size)
    {
        return Some(id);
    }
    tree.children(id)?
        .iter()
        .find_map(|kid| find_box(tree, *kid, size))
}

fn paint_rect_sizes(tree: &mut WidgetTree) -> Vec<Rect> {
    let list = tree.paint();
    let mut transforms = vec![Offset::ZERO];
    let mut rects = Vec::new();
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(*transforms.last().unwrap() + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::Rect { rect, .. } => {
                rects.push(Rect::from_origin_size(
                    rect.origin + *transforms.last().unwrap(),
                    rect.size,
                ));
            }
            _ => {}
        }
    }
    rects
}

fn glyph_run_count(tree: &mut WidgetTree) -> usize {
    tree.paint()
        .commands()
        .iter()
        .filter(|command| matches!(command, PaintCommand::GlyphRun { .. }))
        .count()
}

fn tooltip_glyphs(tree: &mut WidgetTree) -> Vec<Offset> {
    let list = tree.paint();
    let mut transforms = vec![Offset::ZERO];
    let mut origins = Vec::new();
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(*transforms.last().unwrap() + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::GlyphRun { run, .. } => {
                origins.push(run.origin + *transforms.last().unwrap());
            }
            _ => {}
        }
    }
    origins
}

fn descriptions(tree: &mut WidgetTree) -> Vec<String> {
    tree.update_semantics();
    tree.semantics()
        .iter()
        .filter_map(|(_, node)| node.description.clone())
        .collect()
}

fn manual_tip(controller: RawTooltipController) -> Widget {
    RawTooltip::new("Helpful label", Text::new("Trigger"))
        .controller(controller)
        .trigger_mode(TooltipTriggerMode::Manual)
        .into()
}

#[test]
fn overlay_portal_show_hide_cycle_is_idempotent() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, open_portal());
    assert_eq!(tree.transient_surfaces().len(), 1);
    let shown_elements = tree.element_count();

    tree.update(root, open_portal()).expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // Repeated shows neither duplicate surfaces nor leak elements.
    assert_eq!(tree.transient_surfaces().len(), 1);
    assert_eq!(tree.element_count(), shown_elements);

    let hidden: Widget = OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
        .overlay_child(popup_box(30., 40.))
        .role(TransientRole::Menu)
        .show(false)
        .into();
    tree.update(root, hidden).expect("hide");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());

    tree.update(root, open_portal()).expect("reshow");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    assert_eq!(tree.element_count(), shown_elements);
}

#[test]
fn overlay_portal_content_replacement_while_shown() {
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, open_portal());
    let first_id = tree.transient_surfaces()[0].id;
    let old_popup = find_box(&tree, root, Size::new(30., 40.)).expect("popup element");

    let swapped: Widget = OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
        .overlay_child(popup_box(50., 20.))
        .role(TransientRole::Menu)
        .show(true)
        .into();
    tree.update(root, swapped).expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // One surface with the same identity but the new content geometry;
    // the same-type popup updates in place to the new size rather than
    // leaking a stale element.
    let surfaces = tree.transient_surfaces();
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].id, first_id);
    assert_eq!(surfaces[0].content_rect.size, Size::new(50., 20.));
    assert_eq!(find_box(&tree, root, Size::new(50., 20.)), Some(old_popup));
}

#[test]
fn overlay_portal_anchor_moves_without_rebuilding_popup() {
    let popup = popup_box(30., 40.);
    let portal = |inset: f32| {
        Padding::all(
            inset,
            OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
                .overlay_child(popup.clone())
                .role(TransientRole::Menu)
                .show(true),
        )
        .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(portal(8.)).expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 160.)))
        .expect("layout");
    let popup_before = find_box(&tree, root, Size::new(30., 40.)).expect("popup");
    let surface_before = tree.transient_surfaces()[0];

    tree.update(root, portal(24.)).expect("update");
    tree.layout(Constraints::loose(Size::new(200., 160.)))
        .expect("layout");
    let surface_after = tree.transient_surfaces()[0];
    // Anchor and content track the move while popup element and surface
    // identity stay put: no child rebuild, no history cache.
    assert_eq!(surface_after.anchor_rect.origin, Offset::new(24., 24.));
    assert_eq!(surface_after.id, surface_before.id);
    assert_eq!(
        find_box(&tree, root, Size::new(30., 40.)),
        Some(popup_before)
    );
}

#[test]
fn overlay_portal_owner_unmount_clears_state() {
    // Root-type swaps are rejected, so the portal unmounts through a
    // same-type parent update, like any owner dropping its subtree.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Container::with_child(open_portal()).into())
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let content = tree.transient_surfaces()[0].content_rect;
    let popup_center = Offset::new(
        content.origin.x + content.size.width / 2.,
        content.origin.y + content.size.height / 2.,
    );
    // Sanity: the popup is hit-testable while shown.
    let popup_render = tree.hit_test(popup_center).expect("popup hit");
    let popup_element = tree.element_for_render(popup_render).expect("element");

    tree.update(
        root,
        incular_widgets::Container::with_child(Widget::box_(Size::new(200., 200.), Color::WHITE))
            .into(),
    )
    .expect("unmount portal");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // The registry, paint partitions, and hit testing all agree the
    // overlay is gone; the stale popup element resolves nowhere.
    assert!(tree.transient_surfaces().is_empty());
    assert!(!tree.element_exists(popup_element));
    assert!(
        tree.paint()
            .commands()
            .iter()
            .all(|command| !matches!(command, PaintCommand::PushSurfacePartition { .. }))
    );
    let hit = tree
        .hit_test(popup_center)
        .and_then(|render| tree.element_for_render(render));
    assert_ne!(hit, Some(popup_element));
    assert!(hit.is_some_and(|hit| hit != root));
}

#[test]
fn overlay_portal_popup_semantics_exposed() {
    // Popup content participates in semantics with its own bounds while
    // shown; hiding the portal removes the node again.
    let mut tree = WidgetTree::new();
    let root = mount_tight(
        &mut tree,
        OverlayPortal::new(Text::new("Anchor"))
            .overlay_child(
                Semantics::new(popup_box(30., 40.))
                    .role(SemanticRole::Group)
                    .label("popup-group"),
            )
            .role(TransientRole::Menu)
            .show(true)
            .into(),
    );
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(dump.contains("popup-group"));
    assert!(dump.contains("Anchor"));

    let hidden: Widget = OverlayPortal::new(Text::new("Anchor"))
        .overlay_child(
            Semantics::new(popup_box(30., 40.))
                .role(SemanticRole::Group)
                .label("popup-group"),
        )
        .role(TransientRole::Menu)
        .show(false)
        .into();
    tree.update(root, hidden).expect("hide");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    tree.update_semantics();
    let dump = tree.semantics_debug_dump();
    assert!(!dump.contains("popup-group"));
    assert!(dump.contains("Anchor"));
}

#[test]
fn overlay_portal_nested_z_order_inner_wins() {
    // Overlapping popups: the inner popup paints after the outer one and
    // wins hits in the overlap; both surfaces keep parent links.
    let inner: Widget = OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
        .overlay_child(popup_box(60., 60.))
        .anchor_point(Offset::new(10., 10.))
        .placement(TransientPlacement::new())
        .role(TransientRole::Menu)
        .show(true)
        .into();
    let outer: Widget = OverlayPortal::new(Widget::box_(Size::new(80., 80.), Color::WHITE))
        .overlay_child(inner)
        .anchor_point(Offset::new(0., 0.))
        .placement(TransientPlacement::new())
        .role(TransientRole::Popover)
        .show(true)
        .into();
    let mut tree = WidgetTree::new();
    let root = mount_tight(&mut tree, outer);
    let surfaces = tree.transient_surfaces();
    assert_eq!(surfaces.len(), 2);
    let outer_surface = surfaces
        .iter()
        .find(|surface| surface.role == TransientRole::Popover)
        .expect("outer surface");
    let inner_surface = surfaces
        .iter()
        .find(|surface| surface.role == TransientRole::Menu)
        .expect("inner surface");
    assert_eq!(inner_surface.parent, Some(outer_surface.id));

    // The inner popup paints: its full content rect appears in the
    // display list after the outer anchor's rect…
    let rects = paint_rect_sizes(&mut tree);
    let anchor_paint = rects
        .iter()
        .position(|rect| rect.origin == Offset::new(0., 0.) && rect.size == Size::new(80., 80.))
        .expect("outer anchor paints");
    let inner_paint = rects
        .iter()
        .position(|rect| *rect == inner_surface.content_rect)
        .expect("inner popup paints");
    assert!(
        inner_paint > anchor_paint,
        "nested popup must paint above its anchor"
    );
    // …and wins hits in the overlap: the hit element carries the inner
    // popup bounds rather than any outer element.
    let overlap = Offset::new(
        inner_surface.content_rect.origin.x + 2.,
        inner_surface.content_rect.origin.y + 2.,
    );
    let hit = tree
        .hit_test(overlap)
        .and_then(|render| tree.element_for_render(render))
        .expect("overlap hit");
    let hit_bounds = tree.element_bounds(hit).expect("hit bounds");
    assert_eq!(hit_bounds.origin, inner_surface.content_rect.origin);
    assert_eq!(hit_bounds.size, inner_surface.content_rect.size);
    let _ = (root, outer_surface);
}

#[test]
fn overlay_portal_outside_tap_dismisses_with_reason() {
    let reasons = Rc::new(RefCell::new(Vec::new()));
    let observed = reasons.clone();
    let portal: Widget = OverlayPortal::new(Widget::box_(Size::new(30., 20.), Color::WHITE))
        .overlay_child(popup_box(60., 40.))
        .placement(TransientPlacement::new())
        .role(TransientRole::Menu)
        .dismiss_policy(TransientDismissPolicy::interactive())
        .on_dismiss(move |reason| observed.borrow_mut().push(reason))
        .show(true)
        .into();
    let mut tree = WidgetTree::new();
    let root = tree.mount(portal).expect("mount");
    tree.layout(Constraints::tight(Size::new(240., 240.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    // Production dismissal path: one outside pointer dismisses once with
    // its reason; repeating it reports nothing further.
    assert_eq!(
        tree.dismiss_transients_for_pointer(Offset::new(220., 220.)),
        1
    );
    assert_eq!(
        *reasons.borrow(),
        vec![TransientDismissReason::OutsidePointer]
    );
    // Dismissal is a controlled callback, not a silent state mutation:
    // the surface persists until the owner rebuilds hidden.
    assert_eq!(tree.transient_surfaces().len(), 1);
    let hidden: Widget = OverlayPortal::new(Widget::box_(Size::new(30., 20.), Color::WHITE))
        .overlay_child(popup_box(60., 40.))
        .role(TransientRole::Menu)
        .show(false)
        .into();
    tree.update(root, hidden).expect("hide after dismiss");
    tree.layout(Constraints::tight(Size::new(240., 240.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
    assert_eq!(
        tree.dismiss_transients_for_pointer(Offset::new(220., 220.)),
        0
    );
    assert_eq!(reasons.borrow().len(), 1);
}

#[test]
fn overlay_portal_options_matrix() {
    // Presentations, roles, placements, and anchor overrides all reach
    // the retained surface snapshot.
    for presentation in [TransientPresentation::Auto, TransientPresentation::Overlay] {
        let mut tree = WidgetTree::new();
        mount_tight(
            &mut tree,
            OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
                .overlay_child(popup_box(30., 40.))
                .presentation(presentation)
                .show(true)
                .into(),
        );
        assert_eq!(tree.transient_surfaces()[0].presentation, presentation);
    }
    for role in [
        TransientRole::Popover,
        TransientRole::Menu,
        TransientRole::ContextMenu,
        TransientRole::ComboBox,
        TransientRole::Tooltip,
    ] {
        let mut tree = WidgetTree::new();
        mount_tight(
            &mut tree,
            OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
                .overlay_child(popup_box(30., 40.))
                .role(role)
                .show(true)
                .into(),
        );
        assert_eq!(tree.transient_surfaces()[0].role, role);
    }
    // Placement offsets and anchor overrides move the content rect.
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
            .overlay_child(popup_box(30., 40.))
            .placement(
                TransientPlacement::new()
                    .alignment_offset(Offset::new(5., 2.))
                    .safe_margin(EdgeInsets::ZERO),
            )
            .show(true)
            .into(),
    );
    assert_eq!(
        tree.transient_surfaces()[0].content_rect.origin,
        Offset::new(5., 12.)
    );
    let mut tree = WidgetTree::new();
    mount_tight(
        &mut tree,
        OverlayPortal::new(Widget::box_(Size::new(20., 10.), Color::WHITE))
            .overlay_child(popup_box(30., 40.))
            .anchor_rect(Rect::from_origin_size(
                Offset::new(70., 55.),
                Size::new(10., 10.),
            ))
            .placement(TransientPlacement::new().safe_margin(EdgeInsets::ZERO))
            .show(true)
            .into(),
    );
    assert_eq!(
        tree.transient_surfaces()[0].anchor_rect.origin,
        Offset::new(70., 55.)
    );
}

#[test]
fn tooltip_show_hide_cycle_and_repeated_commands() {
    let controller = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    let root = mount_sized(&mut tree, manual_tip(controller.clone()), 120., 80.);
    assert!(tree.transient_surfaces().is_empty());
    let hidden_elements = tree.element_count();

    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    let shown_elements = tree.element_count();
    assert!(shown_elements > hidden_elements);

    controller.hide();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
    assert_eq!(tree.element_count(), hidden_elements);

    // Repeated shows neither duplicate surfaces nor leak elements.
    controller.show();
    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    assert_eq!(tree.element_count(), shown_elements);
    let _ = root;
}

#[test]
fn tooltip_controller_replacement_isolates_old() {
    let first = RawTooltipController::new();
    let second = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    let root = mount_sized(&mut tree, manual_tip(first.clone()), 120., 80.);
    first.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);

    // The replacement controller starts hidden and takes over; the old
    // handle's commands no longer reach the overlay.
    tree.update(root, manual_tip(second.clone()))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
    first.hide();
    first.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
    second.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
}

#[test]
fn tooltip_constructors_agree() {
    // Every message-backed constructor exposes its description once shown;
    // the empty form disables the overlay so show commands surface nothing.
    let cases: Vec<(RawTooltip, &str)> = vec![
        (
            RawTooltip::new("Helpful label", Text::new("Trigger")),
            "Helpful label",
        ),
        (
            RawTooltip::with_rich_message(
                incular_widgets::RichText::new(incular_widgets::TextSpan::new("Rich help")),
                Text::new("Trigger"),
            ),
            "Rich help",
        ),
        (
            RawTooltip::from_rich_message(
                incular_widgets::RichText::new(incular_widgets::TextSpan::new("Rich help")),
                Text::new("Trigger"),
            ),
            "Rich help",
        ),
    ];
    for (tooltip, expected) in cases {
        let controller = RawTooltipController::new();
        let mut tree = WidgetTree::new();
        mount_sized(
            &mut tree,
            tooltip
                .controller(controller.clone())
                .trigger_mode(TooltipTriggerMode::Manual)
                .into(),
            120.,
            80.,
        );
        controller.show();
        tree.layout(Constraints::tight(Size::new(120., 80.)))
            .expect("layout");
        assert_eq!(descriptions(&mut tree), vec![expected.to_string()]);
    }
    for make in [
        (|controller: RawTooltipController| {
            RawTooltip::with_builder(
                Text::new("Trigger"),
                incular_widgets::TooltipComponentBuilder::new(|| Text::new("Custom help").into()),
            )
            .controller(controller)
            .trigger_mode(TooltipTriggerMode::Manual)
            .into()
        }) as fn(RawTooltipController) -> Widget,
        (|controller: RawTooltipController| {
            RawTooltip::with_tooltip_builder(
                incular_widgets::TooltipComponentBuilder::new(|| Text::new("Custom help").into()),
                Text::new("Trigger"),
            )
            .controller(controller)
            .trigger_mode(TooltipTriggerMode::Manual)
            .into()
        }) as fn(RawTooltipController) -> Widget,
    ] {
        // Builder content exists only while shown and paints its text;
        // follower-wrapped custom content stays out of the semantic walk
        // while the follower reports hidden (see the plan note).
        let controller = RawTooltipController::new();
        let mut tree = WidgetTree::new();
        mount_sized(&mut tree, make(controller.clone()), 120., 80.);
        controller.show();
        tree.layout(Constraints::tight(Size::new(120., 80.)))
            .expect("layout");
        assert_eq!(tree.transient_surfaces().len(), 1);
        assert_eq!(glyph_run_count(&mut tree), 2);
    }
    let controller = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    mount_sized(
        &mut tree,
        RawTooltip::empty(Text::new("Trigger"))
            .controller(controller.clone())
            .into(),
        120.,
        80.,
    );
    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
}

#[test]
fn tooltip_content_setters_update_message() {
    let controller = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    let root = mount_sized(&mut tree, manual_tip(controller.clone()), 120., 80.);
    assert_eq!(descriptions(&mut tree), vec!["Helpful label".to_string()]);

    tree.update(
        root,
        RawTooltip::new("Second label", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Manual)
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(descriptions(&mut tree), vec!["Second label".to_string()]);

    // Clearing the message disables the overlay while shown.
    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    tree.update(
        root,
        RawTooltip::new("Second label", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Manual)
            .clear_message()
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());

    // Rich and builder content replace the message source.
    tree.update(
        root,
        RawTooltip::new("Second label", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Manual)
            .set_rich_message(incular_widgets::RichText::new(
                incular_widgets::TextSpan::new("Rich help"),
            ))
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(descriptions(&mut tree), vec!["Rich help".to_string()]);
    tree.update(
        root,
        RawTooltip::new("Second label", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Manual)
            .content(incular_widgets::TooltipComponentBuilder::new(|| {
                Text::new("Custom help").into()
            }))
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    // The custom builder drives the overlay and paints its text.
    assert_eq!(tree.transient_surfaces().len(), 1);
    assert_eq!(glyph_run_count(&mut tree), 2);
    // The sibling setter swaps the builder source with the same effect.
    tree.update(
        root,
        RawTooltip::new("Second label", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Manual)
            .set_tooltip_builder(incular_widgets::TooltipComponentBuilder::new(|| {
                Text::new("Custom help").into()
            }))
            .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    assert_eq!(glyph_run_count(&mut tree), 2);
}

#[test]
fn tooltip_ignore_pointer_passes_hits_through() {
    let popup_hit = |ignore: bool| {
        let controller = RawTooltipController::new();
        let mut tree = WidgetTree::new();
        mount_sized(
            &mut tree,
            RawTooltip::new("Hi", Text::new("Trigger"))
                .controller(controller.clone())
                .trigger_mode(TooltipTriggerMode::Manual)
                .ignore_pointer(ignore)
                .into(),
            120.,
            80.,
        );
        controller.show();
        tree.layout(Constraints::tight(Size::new(120., 80.)))
            .expect("layout");
        let content = tree.transient_surfaces()[0].content_rect;
        let center = Offset::new(
            content.origin.x + content.size.width / 2.,
            content.origin.y + content.size.height / 2.,
        );
        let hit = tree
            .hit_test(center)
            .and_then(|render| tree.element_for_render(render));
        (hit, content)
    };
    let (through, content) = popup_hit(false);
    let (passed, _) = popup_hit(true);
    // Without the flag the popup itself answers; with it the hit falls
    // through to whatever is behind (never the popup element).
    assert!(through.is_some());
    assert_ne!(passed, through);
    let _ = content;
}

#[test]
fn tooltip_triggers_enabled_and_durations() {
    // Tap triggers show; a disabled descriptor ignores trigger input.
    let controller = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    mount_sized(
        &mut tree,
        RawTooltip::new("Hi", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Tap)
            .into(),
        120.,
        80.,
    );
    controller.trigger_tap();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);

    let disabled = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    mount_sized(
        &mut tree,
        RawTooltip::new("Hi", Text::new("Trigger"))
            .controller(disabled.clone())
            .trigger_mode(TooltipTriggerMode::Tap)
            .enabled(false)
            .into(),
        120.,
        80.,
    );
    disabled.trigger_tap();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());

    // Custom waits gate hover visibility on the mounted tree; the delay
    // aliases map onto the duration fields pushed to the controller.
    let hover = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    mount_sized(
        &mut tree,
        RawTooltip::new("Hi", Text::new("Trigger"))
            .controller(hover.clone())
            .focus_node(FocusNode::new())
            .wait_duration(Duration::from_millis(50))
            .touch_delay(Duration::from_millis(80))
            .dismiss_delay(Duration::from_millis(20))
            .hover_delay(Duration::from_millis(50))
            .into(),
        120.,
        80.,
    );
    assert_eq!(hover.durations().wait_duration, Duration::from_millis(50));
    assert_eq!(hover.durations().show_duration, Duration::from_millis(80));
    assert_eq!(hover.durations().exit_duration, Duration::from_millis(20));
    let start = Instant::now();
    assert!(!hover.mouse_enter_at(7, start));
    assert!(!hover.tick(start + Duration::from_millis(49)));
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
    assert!(hover.tick(start + Duration::from_millis(50)));
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
}

#[test]
fn tooltip_dismissal_and_callbacks() {
    // Tap-to-dismiss wiring, trigger callbacks, and pointer dismissal
    // through the production controller path.
    let fired = Rc::new(Cell::new(0));
    let observed = fired.clone();
    let controller = RawTooltipController::new();
    let mut tree = WidgetTree::new();
    mount_sized(
        &mut tree,
        RawTooltip::new("Hi", Text::new("Trigger"))
            .controller(controller.clone())
            .trigger_mode(TooltipTriggerMode::Manual)
            .enable_tap_to_dismiss(true)
            .dismissible(true)
            .enable_feedback(true)
            .on_triggered(move || observed.set(observed.get() + 1))
            .into(),
        120.,
        80.,
    );
    assert!(
        RawTooltip::new("Hi", Text::new("Trigger"))
            .enable_feedback(true)
            .feedback_enabled()
    );
    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert_eq!(tree.transient_surfaces().len(), 1);
    assert_eq!(fired.get(), 1);
    controller.dismiss_by_pointer();
    tree.layout(Constraints::tight(Size::new(120., 80.)))
        .expect("layout");
    assert!(tree.transient_surfaces().is_empty());
    assert_eq!(fired.get(), 1, "dismissal must not re-fire the trigger");
}

#[test]
fn tooltip_semantics_options() {
    // Explicit descriptions override the message; exclusion removes the
    // tooltip node while keeping the trigger.
    let mut tree = WidgetTree::new();
    mount_sized(
        &mut tree,
        RawTooltip::new("Hi", Text::new("Trigger"))
            .semantics_tooltip("Spoken help")
            .into(),
        120.,
        80.,
    );
    assert!(descriptions(&mut tree).contains(&"Spoken help".to_string()));

    let mut tree = WidgetTree::new();
    let root = mount_sized(
        &mut tree,
        RawTooltip::new("Hi", Text::new("Trigger"))
            .semantics_tooltip("Spoken help")
            .clear_semantics_tooltip()
            .into(),
        120.,
        80.,
    );
    // Clearing is explicit suppression, not a revert to the message:
    // neither the override nor the message is announced.
    assert!(!descriptions(&mut tree).contains(&"Spoken help".to_string()));
    assert!(!descriptions(&mut tree).contains(&"Hi".to_string()));
    // The trigger text itself stays exposed in both cases.
    tree.update_semantics();
    assert!(tree.semantics_debug_dump().contains("Trigger"));
    let _ = root;

    for widget in [
        RawTooltip::new("Hi", Text::new("Trigger"))
            .exclude_from_semantics(true)
            .into(),
        RawTooltip::new("Hi", Text::new("Trigger"))
            .without_semantics()
            .into(),
    ] {
        let mut tree = WidgetTree::new();
        mount_sized(&mut tree, widget, 120., 80.);
        // The trigger text stays; the tooltip message is gone.
        tree.update_semantics();
        assert!(tree.semantics_debug_dump().contains("Trigger"));
        assert!(!descriptions(&mut tree).contains(&"Hi".to_string()));
    }
}

#[test]
fn tooltip_placement_options() {
    // prefer_below flips the follower side, vertical_offset shifts it,
    // and explicit anchors compose to the same transform. Placement
    // resolves through the linked follower at paint time; the snapshot
    // keeps reporting layout position.
    let placed = |below: bool, offset: f32| {
        let mut tree = WidgetTree::new();
        let controller = RawTooltipController::new();
        tree.mount(
            Padding::new(
                EdgeInsets::only(0., 50., 0., 0.),
                RawTooltip::new("Hi", Text::new("Trigger"))
                    .controller(controller.clone())
                    .trigger_mode(TooltipTriggerMode::Manual)
                    .prefer_below(below)
                    .vertical_offset(offset),
            )
            .into(),
        )
        .expect("mount");
        tree.layout(Constraints::tight(Size::new(120., 120.)))
            .expect("layout");
        controller.show();
        tree.layout(Constraints::tight(Size::new(120., 120.)))
            .expect("layout");
        tooltip_glyphs(&mut tree)
            .into_iter()
            .nth(1)
            .expect("tooltip glyph")
    };
    // Trigger bottom sits at y=71.3 with the trigger top at y=50.
    assert_eq!(placed(true, 8.).y, 79.28125);
    assert_eq!(placed(false, 8.).y, 20.71875);
    assert_eq!(placed(true, 16.).y, 87.28125);

    // Explicit anchors compose: top/bottom pair with the same offset
    // matches the prefer_below computation.
    let anchored = |target: Alignment, follower: Alignment, offset: Offset| {
        let mut tree = WidgetTree::new();
        let controller = RawTooltipController::new();
        tree.mount(
            Padding::new(
                EdgeInsets::only(0., 50., 0., 0.),
                Widget::from(
                    RawTooltip::new("Hi", Text::new("Trigger"))
                        .controller(controller.clone())
                        .trigger_mode(TooltipTriggerMode::Manual)
                        .target_anchor(target)
                        .follower_anchor(follower)
                        .offset(offset),
                ),
            )
            .into(),
        )
        .expect("mount");
        tree.layout(Constraints::tight(Size::new(120., 120.)))
            .expect("layout");
        controller.show();
        tree.layout(Constraints::tight(Size::new(120., 120.)))
            .expect("layout");
        tooltip_glyphs(&mut tree)
            .into_iter()
            .nth(1)
            .expect("tooltip glyph")
    };
    assert_eq!(
        anchored(
            Alignment::BOTTOM_CENTER,
            Alignment::TOP_CENTER,
            Offset::new(0., 8.)
        ),
        placed(true, 8.)
    );
    // The combined setter composes to the same transform as the three
    // separate ones.
    let mut tree = WidgetTree::new();
    let controller = RawTooltipController::new();
    tree.mount(
        Padding::new(
            EdgeInsets::only(0., 50., 0., 0.),
            Widget::from(
                RawTooltip::new("Hi", Text::new("Trigger"))
                    .controller(controller.clone())
                    .trigger_mode(TooltipTriggerMode::Manual)
                    .position_anchors(
                        Alignment::BOTTOM_CENTER,
                        Alignment::TOP_CENTER,
                        Offset::new(0., 8.),
                    ),
            ),
        )
        .into(),
    )
    .expect("mount");
    tree.layout(Constraints::tight(Size::new(120., 120.)))
        .expect("layout");
    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 120.)))
        .expect("layout");
    assert_eq!(
        tooltip_glyphs(&mut tree).into_iter().nth(1),
        Some(placed(true, 8.))
    );
    // A shared layer link resolves like the default one.
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let controller = RawTooltipController::new();
    tree.mount(
        Padding::new(
            EdgeInsets::only(0., 50., 0., 0.),
            Widget::from(
                RawTooltip::new("Hi", Text::new("Trigger"))
                    .controller(controller.clone())
                    .trigger_mode(TooltipTriggerMode::Manual)
                    .layer_link(link),
            ),
        )
        .into(),
    )
    .expect("mount");
    tree.layout(Constraints::tight(Size::new(120., 120.)))
        .expect("layout");
    controller.show();
    tree.layout(Constraints::tight(Size::new(120., 120.)))
        .expect("layout");
    assert_eq!(
        tooltip_glyphs(&mut tree).into_iter().nth(1),
        Some(placed(true, 0.))
    );
}
