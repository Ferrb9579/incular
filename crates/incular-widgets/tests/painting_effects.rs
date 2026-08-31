use incular_core::Color;
use incular_image::ImageHandle;
use incular_rendering::Path;
use incular_widgets::internal::{ImageFit, ImageRepeat, PhysicalModel, PhysicalShape, WidgetKind};
use incular_widgets::{
    Border, BorderDirectional, BorderRadius, BorderRadiusDirectional, BorderSide, BorderStyle,
    BoxDecoration, BoxShadow, ClipRSuperellipse, DecorationImage, GridPaper, ImageFiltered,
    ImageIcon, Radius, RawImage, SnapshotWidget, Text, Widget,
};
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
    assert_eq!(
        raw,
        RawImage::new()
            .width(0.0)
            .height(0.0)
            .scale(0.001)
            .fit(ImageFit::Contain)
            .repeat(ImageRepeat::NoRepeat)
    );

    let icon: Widget = ImageIcon::builder()
        .image(image_handle())
        .size(-2.0)
        .build()
        .into();
    let WidgetKind::Image { width, height, .. } = icon.kind().clone() else {
        panic!("ImageIcon should lower to an Image widget");
    };
    assert_eq!(width, Some(0.0));
    assert_eq!(height, Some(0.0));

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
    let filtered: Widget = filtered.into();
    assert!(matches!(
        filtered.kind().clone(),
        WidgetKind::Blur { sigma_x, sigma_y, .. } if sigma_x == 3.0 && sigma_y == 3.0
    ));

    let snapshot = SnapshotWidget::builder()
        .child(Text::new("snapshot"))
        .build();
    let _: Widget = snapshot.into();

    let clip = ClipRSuperellipse::builder()
        .radius(-4.0)
        .child(Text::new("clip"))
        .build();
    let clip: Widget = clip.into();
    assert!(matches!(
        clip.kind().clone(),
        WidgetKind::ClipRRect { radius, clip_behavior, .. }
            if radius == incular_rendering::CornerRadii::uniform(0.0)
                && clip_behavior == incular_widgets::Clip::AntiAlias
    ));

    let physical = PhysicalModel::builder()
        .color(Color::WHITE)
        .elevation(-2.0)
        .child(Text::new("physical"))
        .build();
    let expected = PhysicalModel::new(Color::WHITE, Text::new("physical"));
    assert_eq!(Widget::from(physical), Widget::from(expected));

    let physical_shape = PhysicalShape::builder()
        .clipper(Arc::new(Path::builder().build()))
        .color(Color::WHITE)
        .elevation(-2.0)
        .child(Text::new("shape"))
        .build();
    let physical_shape: Widget = physical_shape.into();
    assert!(matches!(
        physical_shape.kind().clone(),
        WidgetKind::ClipPath { .. }
    ));

    let grid = GridPaper::builder().child(Text::new("grid")).build();
    let _: Widget = grid.into();
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
    assert_eq!(image.fit, ImageFit::Cover);
    assert_eq!(image.repeat, ImageRepeat::NoRepeat);

    let decoration = BoxDecoration::builder()
        .color(Color::WHITE)
        .image(image)
        .box_shadow([shadow])
        .build();
    assert!(decoration.is_valid());
    assert_eq!(decoration.box_shadow.len(), 1);
}
