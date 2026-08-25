//! Incular Controls Showcase Gallery
//!
//! Demonstrates platform-neutral styled controls and theme tokens:
//! Buttons (Standard, Primary, Ghost, Icon), Text Inputs, Selection (Checkbox, Radio, Switch),
//! Cards, Dividers, Density (Compact, Standard, Comfortable), and Theme Switching (Light/Dark).

use incular::prelude::{
    Application, Column, Container, CrossAxisAlignment, EdgeInsets, Expanded, Row, Signal,
    SingleChildScrollView, Size, SizedBox, Text, TextEditingController, Widget, WindowOptions,
};
use incular_controls::prelude::*;
use incular_controls::{avatar, meter, progress, scroll_area, toast};

fn main() {
    let mut options = WindowOptions::new("Incular Controls Gallery");
    options.initial_logical_size = Size::new(1180.0, 760.0);
    options.minimum_logical_size = Some(Size::new(760.0, 520.0));
    let remember = Signal::new(true);
    let secondary = Signal::new(false);
    let switch_value = Signal::new(true);
    let slider_value = Signal::new(0.65_f32);
    let field_controller = TextEditingController::new();
    let area_controller = TextEditingController::new();
    let app = Application::new_with_options(options, move |_cx| {
        let theme = ControlTheme::dark();
        let remember_value = remember.get();
        let secondary_value = secondary.get();
        let switch_display = switch_value.get();
        let slider_display = slider_value.get();

        let title = Text::new("Controls Gallery").style(theme.typography.heading.clone());
        let description = Text::new("Base UI-inspired, native retained controls")
            .style(theme.typography.small.clone());

        let sidebar = Container::new()
            .width(182.0)
            .padding(EdgeInsets::all(18.0))
            .color(theme.colors.surface)
            .child(
                Column::new([
                    Widget::from(Text::new("INCULAR").style(theme.typography.caption.clone())),
                    Widget::from(Text::new("Controls").style(theme.typography.title.clone())),
                    Widget::from(SizedBox::new().height(20.0)),
                    Widget::from(Text::new("Inputs").style(theme.typography.caption.clone())),
                    Widget::from(GhostButton::new("Text fields")),
                    Widget::from(GhostButton::new("Selection")),
                    Widget::from(SizedBox::new().height(10.0)),
                    Widget::from(Text::new("Navigation").style(theme.typography.caption.clone())),
                    Widget::from(GhostButton::new("Buttons")),
                    Widget::from(GhostButton::new("Tabs & sliders")),
                    Widget::from(SizedBox::new().height(10.0)),
                    Widget::from(Text::new("Feedback").style(theme.typography.caption.clone())),
                    Widget::from(GhostButton::new("Progress & meter")),
                    Widget::from(GhostButton::new("Toasts")),
                    Widget::from(SizedBox::new().height(10.0)),
                    Widget::from(Text::new("Display").style(theme.typography.caption.clone())),
                    Widget::from(GhostButton::new("Cards & avatars")),
                    Widget::from(GhostButton::new("Scroll areas")),
                ])
                .spacing(6.0),
            );

        // 1. Button Suite
        let buttons = Card::new(
            Column::new([
                Widget::from(Text::new("Buttons").style(theme.typography.title.clone())),
                Widget::from(Row::new([
                    Widget::from(
                        Button::new("Standard Button").on_click(|| println!("Standard clicked")),
                    ),
                    Widget::from(SizedBox::new().width(8.0)),
                    Widget::from(
                        PrimaryButton::new("Primary Action")
                            .on_click(|| println!("Primary clicked")),
                    ),
                    Widget::from(SizedBox::new().width(8.0)),
                    Widget::from(
                        GhostButton::new("Ghost Button").on_click(|| println!("Ghost clicked")),
                    ),
                    Widget::from(SizedBox::new().width(8.0)),
                    Widget::from(IconButton::new("⚙").on_click(|| println!("Settings clicked"))),
                ])),
            ])
            .spacing(10.0),
        );

        // 2. Input Fields
        let inputs = Card::new(
            Column::new([
                Widget::from(Text::new("Text Inputs").style(theme.typography.title.clone())),
                Widget::from(Row::new([Widget::from(Expanded::new(
                    TextField::new(field_controller.clone())
                        .placeholder("Type single-line text..."),
                ))])),
                Widget::from(
                    TextArea::new(area_controller.clone())
                        .placeholder("Multiline text editor area..."),
                ),
            ])
            .spacing(10.0),
        );

        // 3. Selection & Toggles
        let selection = Card::new(
            Column::new([
                Widget::from(
                    Text::new("Selection & Toggles").style(theme.typography.title.clone()),
                ),
                Widget::from(Row::new([
                    Widget::from(
                        Checkbox::new(remember_value)
                            .label("Enabled Feature")
                            .on_changed({
                                let remember = remember.clone();
                                move |value| {
                                    remember.set(value);
                                }
                            }),
                    ),
                    Widget::from(SizedBox::new().width(16.0)),
                    Widget::from(
                        Checkbox::new(secondary_value)
                            .label("Disabled Feature")
                            .on_changed({
                                let secondary = secondary.clone();
                                move |value| {
                                    secondary.set(value);
                                }
                            }),
                    ),
                    Widget::from(SizedBox::new().width(24.0)),
                    Widget::from(Switch::new(switch_display).on_changed({
                        let switch_value = switch_value.clone();
                        move |value| {
                            switch_value.set(value);
                        }
                    })),
                ])),
            ])
            .spacing(10.0),
        );

        // 4. Density Preview
        let density_section = Card::new(
            Column::new([
                Widget::from(Text::new("Density Scaling").style(theme.typography.title.clone())),
                Widget::from(Row::new([
                    Widget::from(
                        Button::new("Compact (26px)").style(
                            ButtonStyle::new()
                                .height(26.0)
                                .padding(EdgeInsets::symmetric(8.0, 3.0)),
                        ),
                    ),
                    Widget::from(SizedBox::new().width(8.0)),
                    Widget::from(
                        Button::new("Standard (32px)").style(
                            ButtonStyle::new()
                                .height(32.0)
                                .padding(EdgeInsets::symmetric(12.0, 6.0)),
                        ),
                    ),
                    Widget::from(SizedBox::new().width(8.0)),
                    Widget::from(
                        Button::new("Comfortable (38px)").style(
                            ButtonStyle::new()
                                .height(38.0)
                                .padding(EdgeInsets::symmetric(16.0, 8.0)),
                        ),
                    ),
                ])),
            ])
            .spacing(10.0),
        );

        // Feedback and display controls exercise the compound APIs that are
        // easy to miss when only the classic button/input examples are shown.
        let feedback_section = Card::new(
            Column::new([
                Widget::from(Text::new("Feedback & display").style(theme.typography.title.clone())),
                Widget::from(Text::new("Progress").style(theme.typography.body_emphasis.clone())),
                Widget::from(progress::Root::new().value(0.62).label("Upload progress")),
                Widget::from(Text::new("Meter").style(theme.typography.body_emphasis.clone())),
                Widget::from(meter::Root::new().value(0.78).label("Storage used")),
                Widget::from(Row::new([
                    Widget::from(avatar::Root::new().label("Ada Lovelace")),
                    Widget::from(SizedBox::new().width(10.0)),
                    Widget::from(
                        toast::Root::new("Saved changes")
                            .description("Your preferences are up to date."),
                    ),
                ])),
            ])
            .spacing(10.0),
        );

        let navigation_section = Card::new(
            Column::new([
                Widget::from(
                    Text::new("Navigation & composition").style(theme.typography.title.clone()),
                ),
                Widget::from(Row::new([
                    Widget::from(
                        slider::Root::new()
                            .value(slider_display)
                            .step(0.05)
                            .on_value_change({
                                let slider_value = slider_value.clone();
                                move |value| {
                                    slider_value.set(value);
                                }
                            }),
                    ),
                    Widget::from(SizedBox::new().width(18.0)),
                    Widget::from(
                        tabs::Root::new().child(tabs::List::new(
                            Row::new([
                                Widget::from(tabs::Tab::new("overview", Text::new("Overview"))),
                                Widget::from(tabs::Tab::new("details", Text::new("Details"))),
                            ])
                            .spacing(8.0),
                        )),
                    ),
                ])),
                Widget::from(
                    collapsible::Root::new()
                        .open(true)
                        .child(Text::new("Collapsible panel content")),
                ),
            ])
            .spacing(10.0),
        );

        let scroll_content = Column::new(
            (1..=12).map(|index| Widget::from(Text::new(format!("Scrollable row {index}")))),
        )
        .spacing(6.0);
        let scroll_section = Card::new(
            Column::new([
                Widget::from(Text::new("Scroll area").style(theme.typography.title.clone())),
                Widget::from(
                    Container::new()
                        .height(130.0)
                        .padding(EdgeInsets::all(8.0))
                        .decoration(
                            incular::widgets::BoxDecoration::new()
                                .color(theme.colors.surface_variant)
                                .border(incular::widgets::Border::new(1.0, theme.colors.border))
                                .border_radius(incular::widgets::BorderRadius::circular(5.0)),
                        )
                        .child(scroll_area::Root::new().child(scroll_content)),
                ),
            ])
            .spacing(10.0),
        );

        let page = Column::new([
            Widget::from(title),
            Widget::from(description),
            Widget::from(Divider::new()),
            Widget::from(buttons),
            Widget::from(inputs),
            Widget::from(selection),
            Widget::from(density_section),
            Widget::from(navigation_section),
            Widget::from(feedback_section),
            Widget::from(scroll_section),
        ])
        .spacing(16.0);

        ControlThemeScope::new(
            theme.clone(),
            Container::new().color(theme.colors.background).child(
                Row::new([
                    Widget::from(sidebar),
                    Widget::from(Expanded::new(
                        Container::new()
                            .padding(EdgeInsets::all(24.0))
                            .child(SingleChildScrollView::new(page)),
                    )),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
            ),
        )
        .into()
    });

    incular::run(app.expect("valid controls gallery application"))
        .expect("native controls gallery application");
}
