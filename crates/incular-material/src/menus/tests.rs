use super::*;
use crate::{
    AppBar, Card, FilledButton, FloatingActionButton, MaterialApp, Scaffold, ScaffoldMessenger,
};
use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Size};
use incular_widgets::{Column, Container, ListView, Text, Widget};

#[test]
fn menu_style_merge_keeps_explicit_values() {
    let base = MenuStyle::new()
        .elevation(4.0)
        .padding(EdgeInsets::all(3.0));
    let override_style = MenuStyle::new()
        .elevation(9.0)
        .background_color(Color::WHITE);
    let merged = base.clone().merge(&override_style);
    assert_eq!(merged.elevation, base.elevation);
    assert_eq!(merged.padding, base.padding);
    assert_eq!(merged.background_color, override_style.background_color);
}

#[test]
fn menu_controller_notifies_retained_revision_on_transitions() {
    let controller = MenuController::new();
    let revision = controller.test_revision();
    controller.open();
    assert!(controller.is_open());
    assert!(controller.test_revision() > revision);
    let next = controller.test_revision();
    controller.open();
    assert_eq!(controller.test_revision(), next);
    controller.close();
    assert!(!controller.is_open());
    assert!(controller.test_revision() > next);
}

#[test]
fn dropdown_entry_preserves_value_and_disabled_state() {
    let entry = DropdownMenuEntry::new(7_u32, "Seven").disabled(true);
    assert_eq!(entry.value, 7);
    assert_eq!(entry.label, "Seven");
    assert!(!entry.enabled);
}

#[test]
fn menu_bar_builds_a_retained_material_surface() {
    let children: Vec<Widget> = vec![
        MenuItemButton::label("File").into(),
        SubmenuButton::new(Text::new("Edit"), [MenuItemButton::label("Undo")]).into(),
    ];
    let _: Widget = MenuBar::new(children).spacing(4.0).into();
}

#[test]
fn menu_anchor_opens_in_a_retained_tree() {
    let controller = MenuController::new();
    let widget: Widget = MenuAnchor::new([MenuItemButton::label("New")])
        .controller(controller.clone())
        .child(FilledButton::tonal("Menu"))
        .into();
    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(ListView::new([widget]).into())
        .expect("mount menu anchor");
    tree.layout(Constraints::tight(Size::new(320.0, 240.0)));
    controller.open();
    tree.layout(Constraints::tight(Size::new(320.0, 240.0)));
    tree.update_semantics();
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.label.as_deref() == Some("New"))
    );
}

#[test]
fn menu_anchor_opens_inside_material_scaffold() {
    let controller = MenuController::new();
    let body: Widget = Container::new()
        .color(Color::WHITE)
        .padding(EdgeInsets::all(24.0))
        .child(ListView::new([Card::new(
            Column::new([
                Widget::from(Text::new("Button families")),
                MenuAnchor::new([MenuItemButton::label("New document")])
                    .controller(controller.clone())
                    .child(FilledButton::tonal("Menu"))
                    .into(),
            ])
            .spacing(12.0),
        )]))
        .into();
    let root = MaterialApp::new(ScaffoldMessenger::new(
        Scaffold::new(body)
            .app_bar(AppBar::new(Text::new("Material Workbench")))
            .floating_action_button(FloatingActionButton::extended("Create")),
    ))
    .build();
    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(root).expect("mount material scaffold");
    tree.layout(Constraints::tight(Size::new(1180.0, 820.0)));
    controller.open();
    tree.layout(Constraints::tight(Size::new(1180.0, 820.0)));
    tree.layout(Constraints::tight(Size::new(1180.0, 820.0)));
    let _ = tree.paint();
    tree.update_semantics();
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.label.as_deref() == Some("New document"))
    );
}

#[test]
fn typed_builders_keep_menu_defaults_and_widget_composition() {
    let style = MenuStyle::builder()
        .background_color(Color::WHITE)
        .padding(EdgeInsets::all(4.0))
        .build();
    assert_eq!(style.padding, Some(EdgeInsets::all(4.0)));

    let item = MenuItemButton::builder()
        .child(Text::new("Open"))
        .leading_icon(Text::new("+"))
        .enabled(false)
        .build();
    assert!(!item.test_enabled());
    let _: Widget = item.into();

    let submenu = SubmenuButton::builder()
        .child(Text::new("Edit"))
        .menu_children([MenuItemButton::label("Undo")])
        .build();
    let _: Widget = submenu.into();

    let anchor = MenuAnchor::typed_builder()
        .menu_children([MenuItemButton::label("Close")])
        .child(Text::new("Menu"))
        .build();
    let _: Widget = anchor.into();

    let popup_item = PopupMenuItem::<u32>::builder()
        .child(Text::new("One"))
        .value(1)
        .height(-1.0)
        .build();
    assert_eq!(popup_item.test_height(), 0.0);
    let checked = CheckedPopupMenuItem::<u32>::builder()
        .child(Text::new("Checked"))
        .checked(true)
        .value(2)
        .build();
    let _: Widget = checked.into();

    let popup = PopupMenuButton::<u32>::builder()
        .item_builder(|| vec![PopupMenuItem::label("One").value(1)])
        .child(Text::new("More"))
        .build();
    let _: Widget = popup.into();

    let dropdown = DropdownButton::<u32>::builder()
        .items([DropdownMenuItem::label("One").value(1)])
        .hint(Text::new("Choose"))
        .build();
    let _: Widget = dropdown.into();

    let entry = DropdownMenuEntry::<u32>::builder()
        .value(1_u32)
        .label("One")
        .label_widget(Text::new("Custom"))
        .build();
    let menu = DropdownMenu::builder()
        .entries([entry])
        .leading_icon(Text::new("+"))
        .build();
    let _: Widget = menu.into();
}
