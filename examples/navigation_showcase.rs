//! Stack navigation, retained route transitions, and modal overlay state.
use incular::prelude::*;
use std::time::{Duration, Instant};

fn page(title: &str, color: Color) -> Widget {
    DecoratedBox::new(Widget::column(vec![
        Text::new(title)
            .style(TextStyle {
                size: 32.,
                color: Color::WHITE,
                ..TextStyle::default()
            })
            .into(),
        Text::new("This screen is held by Navigator and presented through a retained transition.")
            .color(Color::rgba(230, 236, 255, 255))
            .into(),
    ]))
    .size(Size::new(520., 230.))
    .background(color)
    .radius(18.)
    .into()
}

fn main() {
    let navigator = Navigator::new();
    let overlay = Overlay::new();
    let revision = Signal::new(0_u32);
    navigator.push(Route::new(
        "home",
        page("Home", Color::rgba(45, 92, 170, 255)),
    ));

    let app_navigator = navigator.clone();
    let app_overlay = overlay.clone();
    let app_revision = revision.clone();
    let app = Application::new(move |_| {
        let _ = app_revision.get();
        let current = app_navigator.current().expect("home route remains mounted");
        let route_count = app_navigator.routes().len();

        let navigate = app_navigator.clone();
        let rerender_after_push = app_revision.clone();
        let show_dialog = app_overlay.clone();
        let rerender_after_dialog = app_revision.clone();
        let show_sheet = app_overlay.clone();
        let rerender_after_sheet = app_revision.clone();
        let pop = app_navigator.clone();
        let close = app_overlay.clone();
        let rerender_after_pop = app_revision.clone();

        let mut layers = vec![Widget::column(vec![
            Text::new("Navigation & overlays")
                .style(TextStyle {
                    size: 30.,
                    color: Color::rgba(255, 222, 143, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new(format!(
                "route: {}   stack depth: {route_count}   overlay entries: {}",
                current.name,
                app_overlay.entries().len()
            ))
            .color(Color::rgba(225, 232, 248, 255))
            .into(),
            current.presented_child(),
            Widget::row(vec![
                RawMaterialButton::new("Push fade + slide")
                    .on_press(move || {
                        let opacity = OpacityController::new();
                        opacity.set_opacity(0.);
                        opacity.animate_to(1., Duration::from_millis(420), Instant::now());
                        let translation = TranslationController::new();
                        translation.set_offset(Offset::new(120., 0.));
                        translation.animate_to(
                            Offset::ZERO,
                            Duration::from_millis(420),
                            Instant::now(),
                        );
                        navigate.push(
                            Route::new("details", page("Details", Color::rgba(117, 73, 178, 255)))
                                .transition(RouteTransition::FadeSlide {
                                    opacity,
                                    translation,
                                }),
                        );
                        rerender_after_push.update(|value| *value += 1);
                    })
                    .into(),
                RawMaterialButton::new("Show dialog")
                    .on_press(move || {
                        show_dialog.show_dialog(Dialog::new(
                            DecoratedBox::new(
                                Text::new("A modal dialog\nUse Close overlay to dismiss it")
                                    .color(Color::WHITE),
                            )
                            .size(Size::new(340., 130.))
                            .background(Color::rgba(66, 62, 94, 255))
                            .radius(16.),
                        ));
                        rerender_after_dialog.update(|value| *value += 1);
                    })
                    .into(),
                RawMaterialButton::new("Show sheet")
                    .on_press(move || {
                        show_sheet.show_bottom_sheet(BottomSheet::new(
                            DecoratedBox::new(Text::new("Bottom sheet entry").color(Color::WHITE))
                                .size(Size::new(420., 90.))
                                .background(Color::rgba(30, 105, 114, 255))
                                .radius(14.),
                        ));
                        rerender_after_sheet.update(|value| *value += 1);
                    })
                    .into(),
                RawMaterialButton::new("Pop / close overlay")
                    .on_press(move || {
                        if close.remove_top().is_none() && pop.can_pop() {
                            let _ = pop.pop();
                        }
                        rerender_after_pop.update(|value| *value += 1);
                    })
                    .into(),
            ]),
        ])];

        for entry in app_overlay.entries() {
            if let Some(barrier) = entry.barrier {
                layers.push(Widget::box_(Size::new(560., 430.), barrier.color));
            }
            layers.push(entry.child);
        }
        Widget::stack(Alignment::CENTER, layers)
    })
    .expect("valid navigation showcase application");

    incular::run(app).expect("native navigation showcase application");
}
