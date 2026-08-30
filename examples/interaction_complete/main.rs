//! Visual verification for retained gesture arbitration and pointer boundaries.
//!
//! Try a horizontal drag in the blue card, a vertical drag in its parent, or
//! pinch with two contacts. The lower cards demonstrate that IgnorePointer
//! reveals a target behind it while AbsorbPointer blocks both its child and
//! the target behind it. Every active gesture stream receives automatic,
//! window-local retained `PointerCapture` until up or cancellation.
use incular::material::RawMaterialButton;
use incular::prelude::*;

fn text(value: impl Into<String>) -> Widget {
    Text::new(value)
        .style(TextStyle {
            size: 16.,
            color: Color::rgba(230, 236, 250, 255),
            ..TextStyle::default()
        })
        .into()
}

fn card(color: Color, child: impl Into<Widget>) -> Widget {
    DecoratedBox::new(child)
        .size(Size::new(560., 112.))
        .background(color)
        .radius(14.)
        .into()
}

#[path = "../support/mod.rs"]
mod example_support;
#[cfg(test)]
#[path = "tests.rs"]
mod example_tests;
mod simulations;

fn main() {
    let horizontal = Signal::new(0_u32);
    let vertical = Signal::new(0_u32);
    let scale = Signal::new(1_f32);
    let behind_ignore = Signal::new(0_u32);
    let absorbed = Signal::new(0_u32);
    let status = Signal::new(String::from("waiting for an arena claim"));

    let app_horizontal = horizontal.clone();
    let app_vertical = vertical.clone();
    let app_scale = scale.clone();
    let app_behind_ignore = behind_ignore.clone();
    let app_absorbed = absorbed.clone();
    let app_status = status.clone();
    let app = Application::new(move |_| {
        let horizontal_value = app_horizontal.get();
        let vertical_value = app_vertical.get();
        let scale_value = app_scale.get();
        let behind_ignore_value = app_behind_ignore.get();
        let absorbed_value = app_absorbed.get();
        let status_value = app_status.get();

        let vertical_signal = app_vertical.clone();
        let vertical_status = app_status.clone();
        let horizontal_signal = app_horizontal.clone();
        let horizontal_status = app_status.clone();
        let scale_signal = app_scale.clone();
        let scale_status = app_status.clone();
        let inner_surface = GestureDetector::new(card(
            Color::rgba(40, 91, 171, 255),
            Padding::all(
                16.,
                text("Nested arena: drag horizontally here, vertically in the parent, or pinch."),
            ),
        ))
        .on_horizontal_drag_update(move |_| {
            horizontal_signal.update(|value| *value += 1);
            horizontal_status.set(
                "horizontal child won; retained pointer capture is active".into(),
            );
        })
        .on_scale_update(move |details| {
            scale_signal.set(details.scale);
            scale_status.set(
                "scale won compatibly across two contacts; capture is window-local".into(),
            );
        });

        let nested_surface = GestureDetector::new(inner_surface)
            .on_vertical_drag_update(move |_| {
                vertical_signal.update(|value| *value += 1);
                vertical_status.set("vertical parent won; retained pointer capture is active".into());
            });

        let behind_signal = app_behind_ignore.clone();
        let ignore_demo = Widget::stack(
            Alignment::CENTER,
            vec![
                GestureDetector::new(card(
                    Color::rgba(55, 138, 100, 255),
                    Padding::all(16., text("Tap target behind IgnorePointer")),
                ))
                .on_tap(move || behind_signal.update(|value| *value += 1))
                .into(),
                IgnorePointer::new(card(
                    Color::rgba(120, 125, 142, 210),
                    Padding::all(16., text("IgnorePointer overlay — taps pass through to green.")),
                ))
                .into(),
            ],
        );

        let blocked_signal = app_absorbed.clone();
        let absorb_demo = Widget::stack(
            Alignment::CENTER,
            vec![
                GestureDetector::new(card(
                    Color::rgba(124, 71, 90, 255),
                    Padding::all(16., text("This target is behind AbsorbPointer and cannot be tapped.")),
                ))
                .on_tap(move || blocked_signal.update(|value| *value += 1))
                .into(),
                AbsorbPointer::new(card(
                    Color::rgba(155, 80, 67, 255),
                    Padding::all(16., text("AbsorbPointer overlay — normal child and behind targets are blocked.")),
                ))
                .into(),
            ],
        );

        let reset_horizontal = app_horizontal.clone();
        let reset_vertical = app_vertical.clone();
        let reset_scale = app_scale.clone();
        let reset_behind = app_behind_ignore.clone();
        let reset_absorbed = app_absorbed.clone();
        let reset_status = app_status.clone();
        Padding::all(
            20.,
            Widget::column(vec![
                Text::new("Completed interaction primitives")
                    .style(TextStyle {
                        size: 30.,
                        color: Color::rgba(255, 224, 145, 255),
                        ..TextStyle::default()
                    })
                    .into(),
                text(format!(
                    "horizontal: {horizontal_value}  vertical: {vertical_value}  scale: {scale_value:.2}×"
                )),
                text(format!(
                    "IgnorePointer behind taps: {behind_ignore_value}  AbsorbPointer behind taps: {absorbed_value}"
                )),
                text(format!("status: {status_value}")),
                nested_surface.into(),
                ignore_demo,
                absorb_demo,
                RawMaterialButton::new("Reset interaction status")
                    .on_press(move || {
                        reset_horizontal.set(0);
                        reset_vertical.set(0);
                        reset_scale.set(1.);
                        reset_behind.set(0);
                        reset_absorbed.set(0);
                        reset_status.set("waiting for an arena claim".into());
                    })
                    .into(),
            ]),
        )
        .into()
    })
    .expect("valid interaction gallery application");

    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native interaction gallery application");
}
