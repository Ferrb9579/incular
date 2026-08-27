//! Compound, Base UI-inspired controls gallery.
//!
//! Run with `cargo run -p incular --example base_ui_controls` to inspect the
//! neutral preset, stateful indicators, and transparent compound parts.

use incular::prelude::*;
use incular_controls::Button as ControlButton;
use incular_controls::Card as ControlsCard;
use incular_controls::prelude::*;
use incular_widgets::internal::TextEditingController;

fn main() {
    // These values deliberately live outside the application builder.  The
    // retained runtime re-runs the builder when a signal changes, giving the
    // controls a real controlled-state source instead of one-shot print-only
    // callbacks.
    let remember = Signal::new(true);
    let pinned = Signal::new(true);
    let slider_value = Signal::new(0.65_f32);
    let selected_tab = Signal::new(0_u8);
    let editor = TextEditingController::new();
    let mut options = WindowOptions::new("Incular Controls — compound controls");
    options.initial_logical_size = Size::new(900.0, 720.0);
    options.minimum_logical_size = Some(Size::new(680.0, 520.0));
    let app = Application::new_with_options(options, move |_cx| {
        let remember_value = remember.get();
        let pinned_value = pinned.get();
        let slider_display = slider_value.get();
        let tab = selected_tab.get();

        let header = ControlsCard::new(
            Column::new([
                Widget::from(
                    Text::new("Compound controls").style(TextStyle::new().font_size(24.0).bold()),
                ),
                Widget::from(
                    Text::new("Accessible, stateful controls with retained layout").style(
                        TextStyle::new()
                            .font_size(13.0)
                            .color(Color::rgba(164, 172, 190, 255)),
                    ),
                ),
            ])
            .spacing(6.0),
        );

        let actions = ControlsCard::new(
            Column::new([
                Widget::from(
                    Text::new("Button variants").style(TextStyle::new().font_size(14.0).bold()),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(
                            ControlButton::new("Default").on_click(|| println!("default")),
                        ),
                        Widget::from(
                            PrimaryButton::new("Primary").on_click(|| println!("primary")),
                        ),
                        Widget::from(GhostButton::new("Ghost").on_click(|| println!("ghost"))),
                    ])
                    .spacing(8.0),
                ),
            ])
            .spacing(10.0),
        );

        let remember_signal = remember.clone();
        let pinned_signal = pinned.clone();
        let selection = ControlsCard::new(
            Column::new([
                Widget::from(
                    Text::new("Selection and toggles")
                        .style(TextStyle::new().font_size(14.0).bold()),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(
                            checkbox::Root::new()
                                .checked(remember_value)
                                .label("Remember me")
                                .on_checked_change({
                                    let remember = remember_signal.clone();
                                    move |state| {
                                        remember.set(state.is_checked());
                                    }
                                }),
                        ),
                        Widget::from(switch::Root::new().checked(pinned_value).on_checked_change(
                            {
                                let pinned = pinned_signal.clone();
                                move |value| {
                                    pinned.set(value);
                                }
                            },
                        )),
                        Widget::from(
                            toggle::Toggle::new("Pin")
                                .pressed(pinned_value)
                                .on_pressed_change({
                                    let pinned = pinned.clone();
                                    move |value| {
                                        pinned.set(value);
                                    }
                                }),
                        ),
                    ])
                    .spacing(16.0),
                ),
                Widget::from(
                    Text::new(format!(
                        "Remember me: {}    Pin: {}",
                        if remember_value { "on" } else { "off" },
                        if pinned_value { "on" } else { "off" }
                    ))
                    .style(
                        TextStyle::new()
                            .font_size(12.0)
                            .color(Color::rgba(164, 172, 190, 255)),
                    ),
                ),
            ])
            .spacing(10.0),
        );

        let project_field = ControlsCard::new(
            field::Root::new()
                .label("Project name")
                .description("Used in the title bar")
                .child(
                    Input::new(editor.clone())
                        .size(Size::new(520.0, 0.0))
                        .placeholder("Untitled"),
                ),
        );

        let slider_signal = slider_value.clone();
        let slider = ControlsCard::new(
            Column::new([
                Widget::from(Text::new("Slider").style(TextStyle::new().font_size(14.0).bold())),
                Widget::from(
                    Row::new([
                        Widget::from(
                            slider::Root::new()
                                .value(slider_display)
                                .step(0.05)
                                .on_value_change({
                                    let slider = slider_signal.clone();
                                    move |value| {
                                        slider.set(value);
                                    }
                                }),
                        ),
                        Widget::from(Text::new(format!("{slider_display:.2}"))),
                    ])
                    .spacing(14.0),
                ),
            ])
            .spacing(10.0),
        );

        let overview = Text::new(if tab == 0 {
            "Overview is selected. Click Details to switch tabs."
        } else {
            "Details is selected. Click Overview to switch back."
        });
        let selected_tab_signal = selected_tab.clone();
        let tabs = ControlsCard::new(
            Column::new([
                Widget::from(Text::new("Tabs").style(TextStyle::new().font_size(14.0).bold())),
                Widget::from(
                    Row::new([
                        Widget::from(
                            ControlButton::new("Overview")
                                .style(ButtonStyle::new().variant(ButtonVariant::Primary))
                                .on_click({
                                    let selected = selected_tab_signal.clone();
                                    move || {
                                        selected.set(0);
                                    }
                                }),
                        ),
                        Widget::from(
                            ControlButton::new("Details")
                                .style(ButtonStyle::new().variant(ButtonVariant::Ghost))
                                .on_click({
                                    let selected = selected_tab.clone();
                                    move || {
                                        selected.set(1);
                                    }
                                }),
                        ),
                    ])
                    .spacing(8.0),
                ),
                Widget::from(overview),
            ])
            .spacing(10.0),
        );

        let content =
            Container::new()
                .padding(EdgeInsets::all(28.0))
                .child(SingleChildScrollView::new(
                    Column::new([
                        Widget::from(header),
                        Widget::from(actions),
                        Widget::from(selection),
                        Widget::from(project_field),
                        Widget::from(slider),
                        Widget::from(tabs),
                    ])
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .spacing(16.0),
                ));

        ControlThemeScope::new(ControlTheme::dark(), content).into()
    });

    incular::run(app.expect("valid Base UI controls application"))
        .expect("native Base UI controls application");
}
