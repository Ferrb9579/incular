use incular::prelude::*;

struct SampleCustomPainter;

impl CustomPainter for SampleCustomPainter {
    fn paint(&self, size: Size) -> DisplayList {
        let mut canvas = Canvas::default();
        canvas.rect(
            Rect::from_origin_size(Offset::ZERO, size),
            Color::rgba(0, 150, 255, 255),
        );
        canvas.finish()
    }

    fn should_repaint(&self, _old_delegate: &Self) -> bool {
        false
    }
}

#[test]
fn test_painting_and_clipping_compile_contract() {
    let custom_paint: Widget =
        CustomPaint::from_painter(Size::new(200.0, 200.0), SampleCustomPainter).into();

    let clip_rrect: Widget =
        ClipRRect::new(BorderRadius::circular(16.0), SizedBox::shrink()).into();

    let clip_oval: Widget = ClipOval::new(SizedBox::shrink()).into();

    let clip_rect: Widget = ClipRect::new(SizedBox::shrink()).into();

    let _ = (custom_paint, clip_rrect, clip_oval, clip_rect);
}
