use incular::prelude::*;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    // A valid encoded PNG deliberately exercises asset decode before the
    // renderer creates its retained texture; the checker below also exercises
    // generated straight-alpha RGBA pixels and nearest sampling.
    const EMBEDDED_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2, 8, 6,
        0, 0, 0, 114, 182, 13, 36, 0, 0, 0, 24, 73, 68, 65, 84, 120, 156, 5, 193, 129, 1, 0, 0, 4,
        192, 160, 248, 220, 229, 83, 34, 105, 71, 226, 30, 63, 110, 6, 127, 180, 47, 0, 167, 0, 0,
        0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let encoded = ImageHandle::embedded(EMBEDDED_PNG).expect("valid embedded PNG");
    let checker = ImageHandle::from_rgba8(
        2,
        2,
        [
            255, 90, 90, 255, 60, 60, 60, 255, 60, 60, 60, 255, 255, 90, 90, 255,
        ],
    )
    .expect("valid generated image");
    let app = Application::new(move |_| {
        Widget::column(vec![
            Widget::text("Decoded PNG, then shared generated image textures:"),
            Image::new(encoded.clone()).width(128.).height(32.).into(),
            Image::new(checker.clone())
                .width(128.)
                .fit(BoxFit::Contain)
                .into(),
            Image::new(checker.clone())
                .width(128.)
                .height(64.)
                .fit(BoxFit::Cover)
                .into(),
            Widget::text("The same shared ImageHandle is used three times."),
            Image::new(checker.clone())
                .width(64.)
                .height(64.)
                .fit(BoxFit::Fill)
                .sampling(ImageSampling::Nearest)
                .into(),
        ])
    });
    let app = app.expect("build image app");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("run images example");
}
