//! Visual coverage for the retained layout-completeness widgets.
use incular::prelude::*;
use incular::widgets::internal::ScrollView;

const SURFACE: Color = Color::rgba(37, 45, 64, 255);

fn label(value: &str) -> Widget {
    Text::new(value)
        .style(TextStyle {
            size: 15.,
            color: Color::WHITE,
            ..TextStyle::default()
        })
        .into()
}

fn panel(child: impl Into<Widget>) -> Widget {
    DecoratedBox::new(Padding::all(10., child))
        .background(SURFACE)
        .border(Border::new(1., Color::rgba(95, 112, 145, 255)))
        .radius(10.)
        .into()
}

fn tile(size: Size, color: Color) -> Widget {
    DecoratedBox::new(Widget::box_(size, Color::TRANSPARENT))
        .background(color)
        .radius(7.)
        .into()
}

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let scroll = ScrollController::new();
    let app = Application::new(move |_| {
        ScrollView::vertical(scroll.clone(), Padding::all(16., Column::new(vec![
            Text::new("Incular complete layout gallery")
                .style(TextStyle { size: 28., color: Color::rgba(122, 196, 255, 255), ..TextStyle::default() })
                .into(),
            label("Limits, overflow, flex parent data, stack parent data, and compositor affine fitting."),
            panel(Column::new(vec![
                label("LimitedBox (height cap applies in this unbounded scroll axis)"),
                LimitedBox::new(tile(Size::new(480., 96.), Color::rgba(60, 144, 199, 255))).max_width(360.).max_height(48.).into(),
                label("OverflowBox reports available width while its child paints wider"),
                SizedBox::from_size(Size::new(300., 48.)).child(OverflowBox::new(tile(Size::new(460., 42.), Color::rgba(168, 80, 166, 255))).min_width(460.).max_width(460.)).into(),
            ])),
            panel(Column::new(vec![
                label("Expanded + Flexible + Spacer in a bounded row"),
                SizedBox::from_size(Size::new(360., 58.)).child(Row::new(vec![
                    Expanded::new(tile(Size::new(40., 42.), Color::rgba(56, 157, 126, 255))).flex(2).into(),
                    Flexible::new(tile(Size::new(70., 42.), Color::rgba(218, 147, 64, 255))).flex(1).into(),
                    Spacer::new().flex(1).into(),
                    tile(Size::new(42., 42.), Color::rgba(191, 78, 104, 255)),
                ])).into(),
            ])),
            panel(Column::new(vec![
                label("Positioned overlays and IndexedStack's active child"),
                SizedBox::from_size(Size::new(360., 138.)).child(Stack::new(vec![
                    tile(Size::new(330., 112.), Color::rgba(47, 91, 168, 255)),
                    Positioned::new(label("top-left")).left(14.).top(12.).into(),
                    Positioned::new(label("bottom-right")).right(14.).bottom(12.).into(),
                    IndexedStack::new(vec![label("inactive branch"), Text::new("active branch").style(TextStyle { size: 24., color: Color::WHITE, ..TextStyle::default() }).into()]).alignment(Alignment::CENTER).index(1).into(),
                ]).alignment(Alignment::CENTER)).into(),
            ])),
            panel(Column::new(vec![
                label("FittedBox + arbitrary retained affine Transform"),
                SizedBox::from_size(Size::new(360., 115.)).child(FittedBox::new(Transform::rotation(-0.12, tile(Size::new(220., 60.), Color::rgba(81, 169, 137, 255)))).fit(BoxFit::Contain).alignment(Alignment::CENTER)).into(),
            ])),
            panel(LayoutBuilder::new(|_, constraints| {
                let width = if constraints.max_width.is_finite() { constraints.max_width.round() } else { 0. };
                DecoratedBox::new(Padding::all(8., label(&format!("LayoutBuilder observed max width: {width}px"))))
                    .background(Color::rgba(82, 101, 157, 255))
                    .radius(7.)
                    .into()
            })),
        ])))
    })
    .expect("valid layout-complete application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native layout-complete application");
}
