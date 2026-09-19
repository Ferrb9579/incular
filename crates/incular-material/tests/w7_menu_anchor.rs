use incular_config::{Clip, Constraints};
use incular_core::{Offset, Size};
use incular_material::{MenuAnchor, MenuController, MenuItemButton, MenuStyle};
use incular_widgets::{MouseCursor, Text, internal::WidgetTree};
use std::time::{Duration, Instant};

#[test]
fn menu_anchor_policies_reach_overlay_layout_clip_and_lifetime() {
    let anchor = MenuAnchor::new([MenuItemButton::label("Close")])
        .child(Text::new("Menu"))
        .open(true)
        .clip_behavior(Clip::AntiAlias)
        .cross_axis_unconstrained(true)
        .use_root_overlay(true);
    let mut tree = WidgetTree::new();
    tree.mount(anchor.into()).expect("mount menu anchor");
    tree.layout(Constraints::tight(Size::new(320.0, 180.0)))
        .expect("layout open menu");
    assert_eq!(tree.transient_surfaces().len(), 1);
}

#[test]
fn menu_anchor_animated_open_close_retains_overlay_until_exit_finishes() {
    let controller = MenuController::new();
    let anchor = MenuAnchor::new([MenuItemButton::label("Close")])
        .child(Text::new("Menu"))
        .controller(controller.clone())
        .animated(true)
        .open(true);
    let mut tree = WidgetTree::new();
    tree.mount(anchor.into()).expect("mount animated menu");
    tree.layout(Constraints::tight(Size::new(320.0, 180.0)))
        .expect("layout opening menu");
    assert_eq!(tree.transient_surfaces().len(), 1);

    let opened = Instant::now();
    tree.update_compositor(opened)
        .expect("start retained menu transition");
    tree.update_compositor(opened + Duration::from_millis(150))
        .expect("finish retained menu entrance");
    tree.layout(Constraints::tight(Size::new(320.0, 180.0)))
        .expect("settle open menu");
    assert_eq!(tree.transient_surfaces().len(), 1);

    controller.close();
    tree.layout(Constraints::tight(Size::new(320.0, 180.0)))
        .expect("start closing menu");
    assert_eq!(
        tree.transient_surfaces().len(),
        1,
        "closing animation keeps the transient mounted"
    );

    let closing = Instant::now();
    tree.update_compositor(closing)
        .expect("tick menu close transition");
    tree.update_compositor(closing + Duration::from_millis(150))
        .expect("finish menu close transition");
    tree.layout(Constraints::tight(Size::new(320.0, 180.0)))
        .expect("remove closed menu");
    assert!(tree.transient_surfaces().is_empty());
}

#[test]
fn menu_style_constraints_and_cursor_reach_the_transient_surface() {
    let anchor = MenuAnchor::new([MenuItemButton::label("Item")])
        .child(Text::new("Menu"))
        .open(true)
        .style(
            MenuStyle::new()
                .minimum_size(Size::new(180.0, 96.0))
                .mouse_cursor("grab"),
        );
    let mut tree = WidgetTree::new();
    tree.mount(anchor.into()).expect("mount styled menu");
    tree.layout(Constraints::tight(Size::new(360.0, 240.0)))
        .expect("layout styled menu");

    let surface = tree
        .transient_surfaces()
        .into_iter()
        .next()
        .expect("visible menu surface");
    assert!(surface.desired_size.width >= 180.0);
    assert!(surface.desired_size.height >= 96.0);
    let point = Offset::new(
        surface.content_rect.origin.x + surface.content_rect.size.width * 0.5,
        surface.content_rect.origin.y + surface.content_rect.size.height * 0.5,
    );
    assert_eq!(tree.mouse_cursor_at(point), MouseCursor::Grab);
}
