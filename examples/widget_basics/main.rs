//! Widget Basics Example
//! Demonstrates unstyled core primitives, constraint semantics, and decoupled styled components.

#[cfg(feature = "controls")]
use incular::controls_prelude::PrimaryButton;
#[cfg(feature = "material")]
use incular::material_prelude::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::SplitView;

#[cfg(feature = "material")]
fn raw_material_button(child: impl Into<Widget>, on_click: impl Fn() + 'static) -> Widget {
    RawMaterialButton::with_child(child)
        .on_click(on_click)
        .into()
}

#[cfg(feature = "material")]
fn neutral_button() -> Widget {
    raw_material_button(
        Text::new("Neutral Button (Unstyled)")
            .style(TextStyle::new().font_size(13.0).color(Color::WHITE)),
        || println!("Clicked unstyled button"),
    )
}

#[cfg(all(not(feature = "material"), feature = "controls"))]
fn neutral_button() -> Widget {
    PrimaryButton::builder()
        .label("Neutral Button (Control)")
        .on_click(|| println!("Clicked control button"))
        .build()
        .into()
}

#[cfg(not(any(feature = "material", feature = "controls")))]
fn neutral_button() -> Widget {
    Text::new("Neutral Button (optional features disabled)").into()
}

fn styled_button_content() -> Container {
    Container::builder()
        .padding(EdgeInsets::symmetric(14.0, 8.0))
        .color(Color::rgba(60, 110, 220, 255))
        .child(
            Text::new("Styled Action Button").style(
                TextStyle::new()
                    .font_size(13.0)
                    .font_weight(FontWeight::BOLD)
                    .color(Color::WHITE),
            ),
        )
        .build()
}

#[cfg(feature = "material")]
fn styled_button() -> Widget {
    raw_material_button(styled_button_content(), || {
        println!("Clicked styled action button")
    })
}

#[cfg(all(not(feature = "material"), feature = "controls"))]
fn styled_button() -> Widget {
    PrimaryButton::builder()
        .child(styled_button_content())
        .on_click(|| println!("Clicked styled control button"))
        .build()
        .into()
}

#[cfg(not(any(feature = "material", feature = "controls")))]
fn styled_button() -> Widget {
    styled_button_content().into()
}

#[cfg(feature = "controls")]
fn control_builder_demo() -> Widget {
    PrimaryButton::builder()
        .child(
            Text::new("Typed control button")
                .style(TextStyle::new().font_size(12.0).color(Color::WHITE)),
        )
        .on_click(|| println!("Clicked typed control button"))
        .build()
        .into()
}

#[cfg(not(feature = "controls"))]
fn control_builder_demo() -> Widget {
    Text::new("Controls feature disabled").into()
}

#[cfg(feature = "material")]
fn material_widget_demo() -> Widget {
    raw_material_button(
        Text::new("Raw Material action")
            .style(TextStyle::new().font_size(12.0).color(Color::WHITE)),
        || println!("Clicked raw Material action"),
    )
}

#[cfg(not(feature = "material"))]
fn material_widget_demo() -> Widget {
    Text::new("Material feature disabled").into()
}

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let app = Application::new(|_cx| {
        // 1. Unstyled neutral Button: sizes strictly to its child, transparent background.
        let neutral_button = neutral_button();

        // 2. Fluent button composition: the visual surface is a generic Container child.
        let styled_button = styled_button();

        // 3. A typed Container can contain Text, a Controls widget, and a Material widget.
        //    The child remains generic `Widget`; no component-specific child type is needed.
        let composition_demo = Container::builder()
            .padding(EdgeInsets::all(10.0))
            .color(Color::rgba(34, 38, 50, 255))
            .child(
                Column::builder()
                    .children(vec![
                        Text::new("Container::builder composition")
                            .style(TextStyle::new().font_size(12.0).bold())
                            .into(),
                        control_builder_demo(),
                        material_widget_demo(),
                    ])
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .spacing(6.0)
                    .build(),
            )
            .build();

        // 4. Multi-line typography with explicit LineHeight::Multiplier.
        let multi_line_text = Text::new(
            "Line 1: Typography respects multiplier line heights.\n\
             Line 2: Lines are properly spaced without overlap.\n\
             Line 3: 1.5x multiplier means 1.5 * font_size logical pixels.",
        )
        .style(
            TextStyle::new()
                .font_size(14.0)
                .line_height_multiplier(1.5)
                .color(Color::rgba(220, 225, 235, 255)),
        );

        // 5. SplitView with explicit second_extent.
        let left_pane = Container::new()
            .padding(EdgeInsets::all(16.0))
            .color(Color::rgba(30, 32, 38, 255))
            .child(Column::new([
                Widget::from(
                    Text::new("Left Pane (Expanded)").style(
                        TextStyle::new()
                            .font_size(16.0)
                            .font_weight(FontWeight::BOLD)
                            .color(Color::WHITE),
                    ),
                ),
                SizedBox::new().height(12.0).into(),
                multi_line_text.into(),
                SizedBox::new().height(16.0).into(),
                composition_demo.into(),
                SizedBox::new().height(12.0).into(),
                Row::new([
                    neutral_button,
                    SizedBox::new().width(12.0).into(),
                    styled_button,
                ])
                .into(),
            ]));

        let right_pane = Container::new()
            .padding(EdgeInsets::all(16.0))
            .color(Color::rgba(38, 40, 48, 255))
            .child(Column::new([
                Widget::from(
                    Text::new("Right Pane (Fixed 280px)").style(
                        TextStyle::new()
                            .font_size(14.0)
                            .font_weight(FontWeight::BOLD)
                            .color(Color::rgba(140, 180, 255, 255)),
                    ),
                ),
                SizedBox::new().height(8.0).into(),
                Text::new("Uses SplitView::horizontal(...).second_extent(280.0)")
                    .style(
                        TextStyle::new()
                            .font_size(12.0)
                            .color(Color::rgba(180, 185, 195, 255)),
                    )
                    .into(),
            ]));

        let split_demo = SplitView::horizontal(left_pane, right_pane)
            .second_extent(280.0)
            .divider_thickness(6.0)
            .divider_color(Color::rgba(60, 65, 75, 255));

        Container::new()
            .color(Color::rgba(20, 22, 26, 255))
            .child(split_demo)
            .into()
    })
    .expect("valid basics app");

    example_support::spawn_if_requested(app.simulation(), simulations::run);
    if let Err(err) = incular::run(app) {
        eprintln!("Error running widget basics example: {err:?}");
    }
}
