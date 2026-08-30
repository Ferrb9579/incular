//! Visual smoke test for the unified sliver protocol.
//!
//! Scroll this window to see the app bar and the "Pinned section" header stay
//! at the leading edge while the lazy grid moves underneath them.
use incular::prelude::*;
use incular::widgets::internal::{Sliver, SliverAppBar};

const NAVY: Color = Color::rgba(19, 31, 54, 255);
const BLUE: Color = Color::rgba(48, 103, 190, 255);
const TEAL: Color = Color::rgba(28, 148, 142, 255);

fn label(text: impl Into<String>, color: Color, size: f32) -> Widget {
    Text::new(text)
        .style(TextStyle {
            size,
            color,
            ..TextStyle::default()
        })
        .into()
}

fn app_bar() -> Widget {
    DecoratedBox::new(Padding::all(
        16.,
        Row::new([
            label("Incular", Color::WHITE, 24.),
            label(
                "  pinned SliverAppBar",
                Color::rgba(190, 218, 255, 255),
                16.,
            ),
        ]),
    ))
    .size(Size::new(720., 64.))
    .background(NAVY)
    .into()
}

fn pinned_header() -> Widget {
    DecoratedBox::new(Padding::all(
        12.,
        label(
            "Pinned section · scroll to keep this visible",
            Color::WHITE,
            17.,
        ),
    ))
    .size(Size::new(720., 48.))
    .background(TEAL)
    .into()
}

fn tile(index: usize) -> Widget {
    let colors = [
        Color::rgba(48, 103, 190, 255),
        Color::rgba(98, 72, 177, 255),
        Color::rgba(194, 75, 122, 255),
        Color::rgba(204, 128, 53, 255),
    ];
    DecoratedBox::new(Center::new(label(
        format!("Card {index:02}"),
        Color::WHITE,
        18.,
    )))
    .size(Size::new(174., 64.))
    .background(colors[index % colors.len()])
    .radius(10.)
    .into()
}

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let controller = ScrollController::new();
    let app = Application::new(move |_| {
        CustomScrollView::new(vec![
            Box::new(SliverAppBar::new(app_bar()).expanded_height(64.).pinned(true))
                as Box<dyn Sliver>,
            Box::new(SliverToBoxAdapter::new(Padding::all(
                20.,
                Column::new([
                    label("Unified slivers", BLUE, 30.),
                    label(
                        "The content below is lazy. Wheel-scroll to exercise retained viewport updates.",
                        Color::rgba(215, 220, 230, 255),
                        16.,
                    ),
                    SizedBox::from_size(Size::new(1., 18.))
                        .child(Widget::box_(Size::new(1., 1.), Color::TRANSPARENT))
                        .into(),
                ]),
            ))),
            Box::new(SliverPersistentHeader::new(48., pinned_header())),
            Box::new(SliverPadding::new(
                EdgeInsets::all(12.),
                SliverGrid::builder(
                    80,
                    SliverGridDelegate::fixed_cross_axis_count(4).main_axis_extent(76.),
                    tile,
                ),
            )),
        ])
        .controller(controller.clone())
        .into()
    })
    .expect("valid sliver application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native sliver application");
}
