use incular::material::RawMaterialButton;
use incular::prelude::*;

pub const APP_BACKGROUND: Color = Color::rgba(9, 13, 21, 255);
pub const SURFACE: Color = Color::rgba(17, 24, 36, 255);
pub const SURFACE_RAISED: Color = Color::rgba(23, 32, 47, 255);
pub const BORDER: Color = Color::rgba(48, 63, 86, 255);
pub const CONTROL: Color = Color::rgba(42, 54, 74, 255);
pub const CONTROL_ACTIVE: Color = Color::rgba(58, 99, 187, 255);
pub const PRIMARY: Color = Color::rgba(74, 119, 224, 255);
pub const DANGER: Color = Color::rgba(178, 69, 78, 255);
pub const TEXT_PRIMARY: Color = Color::rgba(238, 243, 252, 255);
pub const TEXT_MUTED: Color = Color::rgba(153, 169, 194, 255);
pub const SUCCESS: Color = Color::rgba(74, 205, 151, 255);

pub fn ui_text(value: impl Into<String>, size: f32, color: Color) -> Widget {
    Text::new(value)
        .style(TextStyle {
            size,
            color,
            ..TextStyle::default()
        })
        .into()
}

pub fn gap(width: f32, height: f32) -> Widget {
    Widget::fixed_box(Size::new(width, height), Color::TRANSPARENT)
}

pub fn compact_button(
    label: impl Into<String>,
    selected: bool,
    callback: impl Fn() + 'static,
) -> Widget {
    RawMaterialButton::new(label)
        .size(Size::new(0., 34.))
        .padding(EdgeInsets::symmetric(12., 7.))
        .label_style(TextStyle {
            size: 13.,
            color: TEXT_PRIMARY,
            ..TextStyle::default()
        })
        .color(if selected { CONTROL_ACTIVE } else { CONTROL })
        .on_press(callback)
        .into()
}

pub fn section(title: impl Into<String>, description: impl Into<String>, child: Widget) -> Widget {
    DecoratedBox::new(Padding::all(
        16.,
        Column::new([
            ui_text(title, 16., TEXT_PRIMARY),
            gap(1., 4.),
            ui_text(description, 12., TEXT_MUTED),
            gap(1., 14.),
            child,
        ]),
    ))
    .background(SURFACE_RAISED)
    .border(Border::new(1., BORDER))
    .radius(10.)
    .into()
}
