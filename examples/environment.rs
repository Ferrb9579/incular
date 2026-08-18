//! Resize the window or change desktop scale preference to inspect the typed
//! runtime environment and its logical SafeArea composition.
use incular::prelude::*;

fn main() {
    let app = Application::new(move |cx| {
        let environment = cx.environment();
        let detail = format!(
            "logical viewport: {:.0} × {:.0}\nphysical viewport: {} × {}\nscale: {:.2}\ntext scale: {:.2}\nbrightness: {:?}\nlocale: {}\ndirection: {:?}\nsafe insets: {:?}",
            environment.viewport.width,
            environment.viewport.height,
            environment.physical_width,
            environment.physical_height,
            environment.scale_factor,
            environment.text_scale,
            environment.brightness,
            environment.primary_locale().unwrap_or("platform default"),
            environment.text_direction,
            environment.safe_insets,
        );
        cx.safe_area(
            SafeArea::new(
                Padding::all(
                    24.,
                    DecoratedBox::new(Padding::all(20., Text::new(detail)))
                        .background(Color::rgba(38, 58, 94, 255))
                        .radius(16.),
                ),
            )
            .minimum(EdgeInsets::all(12.)),
        )
    })
    .expect("valid environment application");
    incular::run(app).expect("native environment application");
}
