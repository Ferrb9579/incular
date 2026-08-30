use super::*;
use crate::Text;
use incular_core::Color;
use incular_image::ImageHandle;
use incular_rendering::Path;
use std::sync::Arc;

fn image_handle() -> ImageHandle {
    ImageHandle::from_rgba8(1, 1, vec![255, 255, 255, 255]).unwrap()
}

#[test]
fn effect_builders_preserve_defaults_and_normalize_values() {
    let raw = RawImage::builder()
        .width(-4.0)
        .height(-8.0)
        .scale(0.0)
        .build();
    assert_eq!(raw.width, Some(0.0));
    assert_eq!(raw.height, Some(0.0));
    assert_eq!(raw.scale, 0.001);
    assert_eq!(raw.fit, crate::tree::ImageFit::Contain);
    assert_eq!(raw.repeat, crate::tree::ImageRepeat::NoRepeat);

    let icon = ImageIcon::builder()
        .image(image_handle())
        .size(-2.0)
        .build();
    assert_eq!(icon.size, Some(0.0));
    assert_eq!(icon.color, None);

    let grid = GridPaper::builder().build();
    assert_eq!(grid, GridPaper::default());

    let decoration = BoxDecoration::builder().build();
    assert_eq!(decoration, BoxDecoration::default());
}

#[test]
fn composition_builders_accept_arbitrary_widgets_and_keep_lowering_inputs() {
    let filtered = ImageFiltered::builder()
        .sigma(3.0)
        .child(Text::new("filtered"))
        .build();
    assert_eq!(filtered.controller, None);
    let _: crate::Widget = filtered.into();

    let snapshot = SnapshotWidget::builder()
        .child(Text::new("snapshot"))
        .build();
    let _: crate::Widget = snapshot.into();

    let clip = ClipRSuperellipse::builder()
        .radius(-4.0)
        .child(Text::new("clip"))
        .build();
    assert_eq!(clip.radius, 0.0);
    assert_eq!(clip.clip_behavior, crate::Clip::AntiAlias);
    let _: crate::Widget = clip.into();

    let physical = PhysicalModel::builder()
        .color(Color::WHITE)
        .elevation(-2.0)
        .child(Text::new("physical"))
        .build();
    assert_eq!(physical.elevation, 0.0);
    assert_eq!(physical.shadow_color, Color::rgba(0, 0, 0, 100));
    let _: crate::Widget = physical.into();

    let physical_shape = PhysicalShape::builder()
        .clipper(Arc::new(Path::builder().build()))
        .color(Color::WHITE)
        .elevation(-2.0)
        .child(Text::new("shape"))
        .build();
    assert_eq!(physical_shape.elevation, 0.0);
    let _: crate::Widget = physical_shape.into();

    let grid = GridPaper::builder().child(Text::new("grid")).build();
    let _: crate::Widget = grid.into();
}

#[test]
fn painting_data_builders_match_existing_defaults_and_conversions() {
    let radius = Radius::builder().x(4.0).build();
    assert_eq!(radius, Radius::elliptical(4.0, 0.0));

    let border_radius = BorderRadius::builder().top_left(radius).build();
    assert_eq!(border_radius.top_right, Radius::ZERO);

    let directional = BorderRadiusDirectional::builder().top_start(radius).build();
    assert_eq!(directional.top_end, Radius::ZERO);

    let side = BorderSide::builder().width(-2.0).build();
    assert_eq!(side.width, 0.0);
    assert_eq!(side, BorderSide::new(Color::BLACK, 0.0, BorderStyle::Solid));

    let border = Border::builder()
        .top(side)
        .right(side)
        .bottom(side)
        .left(side)
        .build();
    assert_eq!(border, Border::all(side));

    let directional_border = BorderDirectional::builder().build();
    assert_eq!(directional_border, BorderDirectional::default());

    let shadow = BoxShadow::builder().color(Color::BLACK).build();
    assert_eq!(shadow.blur_radius, 0.0);

    let image = DecorationImage::builder().image(image_handle()).build();
    assert_eq!(image.fit, crate::tree::ImageFit::Cover);
    assert_eq!(image.repeat, crate::tree::ImageRepeat::NoRepeat);

    let decoration = BoxDecoration::builder()
        .color(Color::WHITE)
        .image(image)
        .box_shadow([shadow])
        .build();
    assert!(decoration.is_valid());
    assert_eq!(decoration.box_shadow.len(), 1);
}
