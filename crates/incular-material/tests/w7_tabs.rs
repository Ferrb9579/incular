use incular_config::Constraints;
use incular_core::Size;
use incular_material::{Tab, TabBarView, TabController};
use incular_widgets::{SizedBox, Text, Widget, internal::WidgetTree};

#[test]
fn tab_builder_icon_and_text_reach_measured_content() {
    let tab = Tab::builder().icon(Text::new("ICON")).text("Label").build();
    let widget: Widget = tab.into();

    assert_eq!(widget.semantic_text().as_deref(), Some("ICON Label"));
}

#[test]
fn explicit_tab_child_has_precedence_over_convenience_content() {
    let tab = Tab::builder()
        .child(Text::new("Explicit"))
        .icon(Text::new("ICON"))
        .text("Label")
        .build();
    let widget: Widget = tab.into();

    assert_eq!(widget.semantic_text().as_deref(), Some("Explicit"));
}

#[test]
fn tab_page_extent_follows_real_layout_and_viewport_fraction() {
    let controller = TabController::new(3);
    let view = TabBarView::new([
        SizedBox::new().width(20.0).height(20.0),
        SizedBox::new().width(20.0).height(20.0),
        SizedBox::new().width(20.0).height(20.0),
    ])
    .controller(controller.clone())
    .viewport_fraction(0.5);

    let mut tree = WidgetTree::new();
    tree.mount(view.into()).expect("mount tab view");
    tree.layout(Constraints::tight(Size::new(320.0, 100.0)))
        .expect("layout tab view");

    controller.set_index(1);
    assert!((controller.page_controller().offset() - 160.0).abs() < f32::EPSILON);

    tree.layout(Constraints::tight(Size::new(500.0, 100.0)))
        .expect("relayout tab view");
    assert!((controller.page_controller().offset() - 250.0).abs() < f32::EPSILON);
}

#[test]
fn unchanged_tab_index_does_not_bump_revision() {
    let controller = TabController::new(3);
    let revision = controller.revision();
    controller.set_index(0);
    assert_eq!(controller.revision(), revision);
}
