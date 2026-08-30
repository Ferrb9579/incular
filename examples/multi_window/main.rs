//! Native multi-window retained roots sharing one Signal, Tokio runtime, and
//! GPU device. Close and reopen the auxiliary windows to exercise generation
//! checks and independent environments.
use incular::material::RawMaterialButton;
use incular::prelude::*;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let count = Signal::new(0_u32);
    let primary_count = count.clone();
    let app = Application::new(move |cx| {
        let shared_count = primary_count.clone();
        let counter_opener = cx
            .window_opener()
            .expect("application owns a window opener");
        let inspector_opener = counter_opener.clone();
        let value = primary_count.get();

        Widget::column(vec![
            Text::new("Incular Multi Window")
                .color(Color::rgba(215, 232, 255, 255))
                .into(),
            Padding::all(10., Text::new(format!("Shared count: {value}"))).into(),
            RawMaterialButton::new("Increment shared count")
                .on_press({
                    let count = primary_count.clone();
                    move || count.update(|value| *value += 1)
                })
                .into(),
            RawMaterialButton::new("Open Counter Window")
                .on_press(move || {
                    let count = shared_count.clone();
                    let _ = counter_opener.open_window_with(
                        WindowOptions {
                            title: "Shared Counter".into(),
                            initial_logical_size: Size::new(420., 240.),
                            ..WindowOptions::default()
                        },
                        move |_cx| {
                            let value = count.get();
                            Widget::column(vec![
                                Text::new("Counter Window").into(),
                                Text::new(format!("Shared count: {value}")).into(),
                                RawMaterialButton::new("Increment")
                                    .on_press({
                                        let count = count.clone();
                                        move || count.update(|value| *value += 1)
                                    })
                                    .into(),
                            ])
                        },
                    );
                })
                .into(),
            RawMaterialButton::new("Open Inspector Window")
                .on_press({
                    let inspector_count = primary_count.clone();
                    move || {
                        let count = inspector_count.clone();
                        let _ = inspector_opener.open_window_with(
                            WindowOptions {
                                title: "Window Inspector".into(),
                                initial_logical_size: Size::new(520., 310.),
                                ..WindowOptions::default()
                            },
                            move |cx| {
                                let value = count.get();
                                let viewport = cx.viewport();
                                let scale = cx.scale_factor();
                                Widget::column(vec![
                                    Text::new("Independent window environment").into(),
                                    Text::new(format!(
                                        "logical: {:.0} × {:.0}; scale: {scale:.2}",
                                        viewport.width, viewport.height
                                    ))
                                    .into(),
                                    Text::new(format!("shared count: {value}")).into(),
                                ])
                            },
                        );
                    }
                })
                .into(),
        ])
    })
    .expect("valid multi-window application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native multi-window application");
}
