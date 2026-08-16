//! A direct `Canvas`/`CustomPaint` smoke test for the extracted rendering and
//! image crates.  It uses only generated pixels, clips, and translations.
use incular::{
    core::{Rect, Transform},
    prelude::*,
};
use std::sync::Arc;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_origin_size(Offset::new(x, y), Size::new(width, height))
}

fn generated_tile() -> ImageHandle {
    let mut pixels = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            let light = (x + y) % 2 == 0;
            pixels.extend_from_slice(if light {
                &[255, 220, 108, 255]
            } else {
                &[42, 57, 110, 255]
            });
        }
    }
    ImageHandle::from_rgba8(8, 8, pixels).expect("valid generated tile")
}

fn spark() -> Arc<Path> {
    let mut path = Path::builder();
    path.move_to(Offset::new(0., 30.))
        .quadratic_to(Offset::new(46., 4.), Offset::new(88., 30.))
        .quadratic_to(Offset::new(46., 56.), Offset::new(0., 30.))
        .close();
    Arc::new(path.build())
}

fn artwork() -> DisplayList {
    let tile = generated_tile();
    let mut canvas = Canvas::default();
    canvas.rect(rect(0., 0., 520., 310.), Color::rgba(17, 24, 48, 255));

    let panel = RRect::uniform(rect(24., 26., 472., 188.), 20.);
    canvas.rrect(
        panel,
        LinearGradient {
            start: Offset::new(24., 26.),
            end: Offset::new(496., 214.),
            stops: GradientStops::new(vec![
                GradientStop {
                    offset: 0.,
                    color: Color::rgba(56, 104, 240, 255),
                },
                GradientStop {
                    offset: 0.55,
                    color: Color::rgba(168, 79, 229, 255),
                },
                GradientStop {
                    offset: 1.,
                    color: Color::rgba(42, 201, 171, 255),
                },
            ]),
        },
    );
    canvas.border(panel, Border::new(2., Color::rgba(255, 255, 255, 170)));

    canvas.save_clip_rrect(RRect::uniform(rect(42., 46., 208., 148.), 14.));
    canvas.rect(rect(42., 46., 208., 148.), Color::rgba(12, 18, 43, 255));
    canvas.image_with_sampling(
        tile,
        rect(0., 0., 8., 8.),
        rect(42., 46., 208., 148.),
        ImageSampling::Nearest,
    );
    canvas.restore();

    canvas.save_clip(rect(276., 46., 184., 148.));
    canvas.save_transform(Transform::translation(Offset::new(312., 84.)));
    canvas.fill_path(
        spark(),
        RadialGradient {
            center: Offset::new(44., 30.),
            radius: 55.,
            stops: GradientStops::new(vec![
                GradientStop {
                    offset: 0.,
                    color: Color::WHITE,
                },
                GradientStop {
                    offset: 0.5,
                    color: Color::rgba(255, 224, 92, 255),
                },
                GradientStop {
                    offset: 1.,
                    color: Color::rgba(245, 89, 136, 255),
                },
            ]),
        },
        FillRule::NonZero,
    );
    canvas.stroke_path(
        spark(),
        Color::rgba(255, 255, 255, 220),
        Stroke {
            width: 3.,
            cap: LineCap::Round,
            join: LineJoin::Round,
            ..Stroke::default()
        },
    );
    canvas.restore();
    canvas.restore();

    for (index, color) in [
        Color::rgba(91, 215, 255, 255),
        Color::rgba(255, 186, 94, 255),
        Color::rgba(205, 125, 255, 255),
    ]
    .into_iter()
    .enumerate()
    {
        canvas.rrect(
            RRect::uniform(rect(36. + index as f32 * 154., 240., 132., 44.), 12.),
            color,
        );
    }
    canvas.finish()
}

fn main() {
    let display_list = artwork();
    let app = Application::new(move |_| {
        Widget::column(vec![
            Text::new("Canvas layers")
                .style(TextStyle {
                    size: 28.,
                    color: Color::rgba(255, 230, 165, 255),
                    ..TextStyle::default()
                })
                .into(),
            Text::new("CustomPaint: clipped nearest-sampled pixels, transformed path, gradients, and borders.")
                .color(Color::rgba(210, 220, 245, 255))
                .into(),
            RepaintBoundary::new(CustomPaint::new(Size::new(520., 310.), display_list.clone()))
                .into(),
        ])
    })
    .expect("valid canvas application");
    incular::run(app).expect("native canvas application");
}
