use incular_config::{Clip, Constraints};
use incular_core::Size;
use incular_material::{MenuAnchor, MenuItemButton};
use incular_widgets::{Text, internal::WidgetTree};

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
#[should_panic(expected = "MenuAnchor::animated(true) is not supported")]
fn menu_anchor_animated_true_is_explicitly_rejected_instead_of_ignored() {
    let anchor = MenuAnchor::new([MenuItemButton::label("Close")])
        .child(Text::new("Menu"))
        .animated(true);
    let mut tree = WidgetTree::new();
    tree.mount(anchor.into()).expect("mount must reach builder");
    tree.layout(Constraints::tight(Size::new(320.0, 180.0)))
        .expect("animated policy should be rejected before this succeeds");
}
