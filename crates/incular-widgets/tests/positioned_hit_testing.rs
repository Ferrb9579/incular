use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_widgets::{
    AbsorbPointer, IgnorePointer, Positioned, Stack, Widget,
    internal::{ActionId, WidgetTree, action},
};

#[test]
fn positioned_overlay_defers_to_its_child_hit_policy() {
    for ignoring in [true, false] {
        let overlay: Widget = if ignoring {
            IgnorePointer::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).into()
        } else {
            AbsorbPointer::new(Widget::box_(Size::new(80., 40.), Color::WHITE)).into()
        };
        let mut tree = WidgetTree::new();
        tree.mount(
            Stack::new([
                action(Size::new(80., 40.), Color::WHITE, ActionId(7)),
                Positioned::fill(overlay).into(),
            ])
            .into(),
        )
        .expect("mount");
        tree.layout(Constraints::loose(Size::new(100., 100.)))
            .expect("layout");
        let hit = tree.hit_test(Offset::new(20., 20.)).expect("hit");
        let element = tree.element_for_render(hit).expect("element");
        assert_eq!(
            tree.action_for_element(element),
            ignoring.then_some(ActionId(7))
        );
    }
}
