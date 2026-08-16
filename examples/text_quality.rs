//! Inspect Incular's DPI-aware grayscale text path across micro through huge sizes.
use incular::prelude::*;

fn sample(size: f32, text: &str, color: Color) -> Widget {
    Text::new(format!("{size:>3.0} px  {text}"))
        .style(TextStyle {
            size,
            color,
            ..TextStyle::default()
        })
        .into()
}

fn fractional_position_sample(position: f32, color: Color) -> Widget {
    Widget::padding(
        EdgeInsets {
            left: position,
            top: 0.,
            right: 0.,
            bottom: 0.,
        },
        sample(
            12.,
            &format!("fractional x {position:.2}: Settings  Cancel  1234567890"),
            color,
        ),
    )
}

fn main() {
    let controller = ScrollController::new();
    let app = Application::new(move |_| {
        let pale = Color::rgba(235, 240, 250, 255);
        let cyan = Color::rgba(145, 220, 255, 255);
        let gold = Color::rgba(255, 215, 135, 255);
        let mut rows = vec![
            sample(24., "Incular text-quality matrix", gold),
            sample(
                14.,
                "Micro: H E F I L T  O C G S Q  A V W M N X  m n r a e s  0123456789  .,:;!?()[]{}",
                pale,
            ),
        ];
        for size in [5., 6., 7., 8., 9., 10., 11., 12.] {
            rows.push(sample(
                size,
                "H E F I L T   O C G S Q   A V W M N X   m n r a e s   0123456789   .,:;!?()[]{}",
                pale,
            ));
        }
        rows.push(sample(18., "Ordinary UI text", cyan));
        for size in [13., 14., 16., 18., 20., 24., 32.] {
            rows.push(sample(
                size,
                "The quick brown fox jumps over the lazy dog.  0123456789  HAMBURGEFONS",
                pale,
            ));
        }
        rows.push(sample(18., "Curves, diagonals, and dense lowercase", cyan));
        for size in [10., 12., 14., 16., 18., 20., 24., 32.] {
            rows.push(sample(
                size,
                "AVWXYZ  / \\  X M N     OCGSQ  aceos  03689     minimum  renderer  settings  typography",
                pale,
            ));
        }
        rows.push(sample(18., "Unicode coverage: Latin  العربية  नमस्ते  日本語", cyan));
        rows.push(sample(
            16.,
            "The quick brown fox.  Settings  Cancel  Continue  Save changes  0123456789",
            pale,
        ));
        rows.push(sample(18., "Fractional GPU placement", cyan));
        for position in [0., 0.25, 0.50, 0.75] {
            rows.push(fractional_position_sample(position, pale));
        }
        rows.push(sample(24., "Large type", cyan));
        for size in [48., 64., 96., 128., 192., 256.] {
            rows.push(sample(size, "H O A V W 0123456789", gold));
        }
        rows.push(sample(32., "Huge type", cyan));
        rows.push(sample(384., "H O A V", Color::WHITE));
        ScrollView::vertical(controller.clone(), Widget::column(rows))
    })
    .expect("valid text-quality application");
    incular::run(app).expect("native text-quality application");
}
