//! Scroll policy and nested-delta visual exercise.
//!
//! The inner list is deliberately scrollable inside an outer header/content
//! viewport. At its top or bottom, wheel remainder transfers to the outer
//! viewport exactly once. The variable rows use the retained sliver list path.
use incular::prelude::*;
use incular::widgets::internal::ScrollView;

fn main() {
    let outer = ScrollController::new();
    let inner = ScrollController::new();
    // Applications may drive kinetic completion from their monotonic frame
    // callback with this policy; the retained wheel path uses clamped nested
    // transfer by default.
    let _touch_policy = ScrollPhysics::clamping().bouncing().page_snapping(320.);
    let app = Application::new(move |_| {
        let list: Widget = CustomScrollView::new(vec![Box::new(SliverVariedExtentList::new(
            2_000,
            |item| if item % 3 == 0 { 56. } else { 32. },
            |item| {
                Widget::fixed_box(
                    Size::new(360., if item % 3 == 0 { 56. } else { 32. }),
                    Color::rgba(45, 85 + (item % 4) as u8 * 24, 145, 255),
                )
            },
        )) as Box<dyn Sliver>])
        .controller(inner.clone())
        .into();
        let inner_view: Widget = SizedBox::from_size(Size::new(360., 320.))
            .child(list)
            .into();
        ScrollView::vertical(
            outer.clone(),
            Widget::column(vec![
                Widget::fixed_box(Size::new(360., 120.), Color::rgba(35, 45, 70, 255)),
                inner_view,
                Widget::fixed_box(Size::new(360., 700.), Color::rgba(30, 38, 55, 255)),
            ]),
        )
    })
    .expect("valid scrolling application");
    incular::run(app).expect("native scrolling application");
}
