//! Renderer-independent image-fit behavior tests.

use incular_config::Alignment;
use incular_core::{Offset, Rect, Size};
use incular_widgets::internal::{
    ImageFit, ImageRepeat, image_fit_rects, image_repeat_destinations,
};

#[test]
fn contain_cover_fill_and_scale_down_are_deterministic() {
    let source = Rect::from_origin_size(Offset::ZERO, Size::new(400., 200.));
    let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(200., 200.));
    let (_, contain) = image_fit_rects(source, bounds, ImageFit::Contain, Alignment::CENTER);
    assert_eq!(contain.size, Size::new(200., 100.));
    let (cover_source, cover_destination) =
        image_fit_rects(source, bounds, ImageFit::Cover, Alignment::CENTER);
    assert_eq!(cover_destination, bounds);
    assert_eq!(cover_source.size, Size::new(200., 200.));
    assert_eq!(
        image_fit_rects(source, bounds, ImageFit::Fill, Alignment::CENTER).1,
        bounds
    );
    assert_eq!(
        image_fit_rects(
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            bounds,
            ImageFit::ScaleDown,
            Alignment::CENTER
        )
        .1
        .size,
        Size::new(40., 20.)
    );
}

#[test]
fn repeat_destinations_cover_requested_axes_from_the_fitted_origin() {
    let destination = Rect::from_origin_size(Offset::new(5., 2.), Size::new(10., 4.));
    let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(30., 10.));
    let tiles = image_repeat_destinations(destination, bounds, ImageRepeat::RepeatX);
    assert_eq!(tiles.len(), 4);
    assert_eq!(tiles[0].origin, Offset::new(-5., 2.));
    assert_eq!(tiles[3].origin, Offset::new(25., 2.));
    assert_eq!(
        image_repeat_destinations(destination, bounds, ImageRepeat::NoRepeat),
        vec![destination]
    );
}
