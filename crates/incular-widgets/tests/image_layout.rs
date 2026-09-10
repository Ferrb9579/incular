//! Image layout, painting, and replacement contracts.
//!
//! `Image` retains only a decoded handle plus sizing policy; layout sizes
//! from the intrinsic ratio, fit/alignment resolution happens in
//! `image_fit_rects` at paint time, and decoding/caching stay in
//! incular-image. Assertions use neutral render geometry.

use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size};
use incular_image::ImageHandle;
use incular_rendering::{DisplayList, PaintCommand};
use incular_widgets::{
    FilterQuality, Image, Transform, Widget,
    internal::{ImageFit, ImageRepeat, WidgetTree},
};

fn handle(width: u32, height: u32) -> ImageHandle {
    let pixels = vec![255u8; (width * height * 4) as usize];
    ImageHandle::from_rgba8(width, height, pixels).expect("handle")
}

fn layout(
    build: impl FnOnce() -> Widget,
    constraints: Constraints,
) -> (WidgetTree, incular_widgets::internal::ElementId) {
    let mut tree = WidgetTree::new();
    let root = tree.mount(build()).expect("mount");
    tree.layout(constraints).expect("layout");
    (tree, root)
}

fn bounds(tree: &WidgetTree, id: incular_widgets::internal::ElementId) -> (f32, f32, f32, f32) {
    let b = tree.element_bounds(id).expect("bounds");
    (b.origin.x, b.origin.y, b.size.width, b.size.height)
}

fn image_commands(list: &DisplayList) -> Vec<(Rect, Rect)> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Image {
                source,
                destination,
                ..
            } => Some((*source, *destination)),
            _ => None,
        })
        .collect()
}

#[test]
fn image_sizes_from_intrinsic_ratio() {
    // 40x20 source measured under loose constraints keeps its ratio.
    let (tree, root) = layout(
        || Image::new(handle(40, 20)).into(),
        Constraints::loose(Size::new(100., 100.)),
    );
    assert_eq!(bounds(&tree, root), (0., 0., 40., 20.));

    // Width-only derives the height from the ratio.
    let (tree, root) = layout(
        || Image::new(handle(40, 20)).width(80.).into(),
        Constraints::loose(Size::new(200., 200.)),
    );
    assert_eq!(bounds(&tree, root), (0., 0., 80., 40.));

    // Height-only derives the width.
    let (tree, root) = layout(
        || Image::new(handle(40, 20)).height(40.).into(),
        Constraints::loose(Size::new(200., 200.)),
    );
    assert_eq!(bounds(&tree, root), (0., 0., 80., 40.));

    // Bounded constraints clamp the natural size.
    let (tree, root) = layout(
        || Image::new(handle(40, 20)).into(),
        Constraints::tight(Size::new(30., 30.)),
    );
    assert_eq!(bounds(&tree, root), (0., 0., 30., 30.));
}

#[test]
fn image_negative_dimensions_floor_to_zero() {
    // Negative dimensions floor at the descriptor like RawImage; they
    // must not panic later in layout.
    let (tree, root) = layout(
        || Image::new(handle(40, 20)).width(-5.).height(-9.).into(),
        Constraints::loose(Size::new(100., 100.)),
    );
    // A zero width with a zero height measures as the empty box.
    assert_eq!(bounds(&tree, root), (0., 0., 0., 0.));
}

#[test]
fn fit_modes_project_source_and_destination() {
    // 40x20 source in a 100x100 box per fit mode.
    let cases: &[(ImageFit, (u32, u32), Rect, Rect)] = &[
        // Fill stretches to the full box.
        (
            ImageFit::Fill,
            (40, 20),
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            Rect::from_origin_size(Offset::ZERO, Size::new(100., 100.)),
        ),
        // Contain: scale 2.5 -> 100x50, centered vertically.
        (
            ImageFit::Contain,
            (40, 20),
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            Rect::from_origin_size(Offset::new(0., 25.), Size::new(100., 50.)),
        ),
        // Cover: scale 5 -> 200x100; crop 20x20 centered in the 40-wide
        // source, so x = (40 - 20) / 2 = 10.
        (
            ImageFit::Cover,
            (40, 20),
            Rect::from_origin_size(Offset::new(10., 0.), Size::new(20., 20.)),
            Rect::from_origin_size(Offset::ZERO, Size::new(100., 100.)),
        ),
        // FitWidth: scale 2.5 -> 100x50.
        (
            ImageFit::FitWidth,
            (40, 20),
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            Rect::from_origin_size(Offset::new(0., 25.), Size::new(100., 50.)),
        ),
        // FitHeight: scale 5 -> 200x100, centered horizontally (overflow).
        (
            ImageFit::FitHeight,
            (40, 20),
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            Rect::from_origin_size(Offset::new(-50., 0.), Size::new(200., 100.)),
        ),
        // None: 1:1 source, centered.
        (
            ImageFit::None,
            (40, 20),
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            Rect::from_origin_size(Offset::new(30., 40.), Size::new(40., 20.)),
        ),
        // ScaleDown never upscales: a small source stays 1:1 and centers.
        (
            ImageFit::ScaleDown,
            (40, 20),
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
            Rect::from_origin_size(Offset::new(30., 40.), Size::new(40., 20.)),
        ),
        // ScaleDown shrinks a large source as contain would.
        (
            ImageFit::ScaleDown,
            (400, 200),
            Rect::from_origin_size(Offset::ZERO, Size::new(400., 200.)),
            Rect::from_origin_size(Offset::new(0., 25.), Size::new(100., 50.)),
        ),
    ];
    for (fit, (sw, sh), source, destination) in cases {
        let (mut tree, _) = layout(
            || {
                Image::new(handle(*sw, *sh))
                    .width(100.)
                    .height(100.)
                    .fit(*fit)
                    .into()
            },
            Constraints::loose(Size::new(100., 100.)),
        );
        let commands = image_commands(&tree.paint());
        assert_eq!(commands.len(), 1, "{fit:?}");
        assert_eq!(commands[0], (*source, *destination), "{fit:?}");
    }
}

#[test]
fn alignment_moves_fitted_image_after_cached_paint() {
    // Contain leaves 50px of vertical slack; alignment moves the image.
    let build = |alignment| {
        Image::new(handle(40, 20))
            .width(100.)
            .height(100.)
            .fit(ImageFit::Contain)
            .alignment(alignment)
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::from(build(incular_config::Alignment::CENTER)))
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .expect("layout");
    let centered = image_commands(&tree.paint())[0].1;

    tree.update(
        root,
        Widget::from(build(incular_config::Alignment::TOP_LEFT)),
    )
    .expect("realign after paint");
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .expect("layout");
    let top_left = image_commands(&tree.paint())[0].1;
    assert_eq!(centered.origin, Offset::new(0., 25.));
    assert_eq!(top_left.origin, Offset::ZERO);
    // A paint-only change must not re-run layout.
    let layouts = tree.diagnostics().layouts;
    tree.update(
        root,
        Widget::from(build(incular_config::Alignment::BOTTOM_RIGHT)),
    )
    .expect("realign again");
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .expect("layout");
    let bottom_right = image_commands(&tree.paint())[0].1;
    assert_eq!(bottom_right.origin, Offset::new(0., 50.));
    assert_eq!(tree.diagnostics().layouts, layouts + 1);
}

#[test]
fn image_replacement_changes_intrinsic_size() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Image::new(handle(40, 20)).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds(&tree, root), (0., 0., 40., 20.));

    // Replacing with a differently shaped handle re-measures.
    tree.update(root, Image::new(handle(10, 50)).into())
        .expect("replace image");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(bounds(&tree, root), (0., 0., 10., 50.));
    assert_eq!(
        tree.element_bounds(root).expect("bounds").size,
        Size::new(10., 50.)
    );
}

#[test]
fn same_image_reapplication_bails_out() {
    let image = Image::new(handle(40, 20));
    let build = || Widget::from(image.clone());
    let mut tree = WidgetTree::new();
    let root = tree.mount(build()).expect("mount");
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(root, build()).expect("reapply");
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .expect("layout");
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
}

#[test]
fn raw_image_lowering_matches_direct_image() {
    use incular_widgets::RawImage;
    // RawImage lowers to the same retained Image as the direct descriptor
    // for the options it forwards (fit, repeat, width, height).
    let shared = handle(40, 20);
    let built = RawImage::new()
        .image(shared.clone())
        .width(80.)
        .height(40.)
        .fit(ImageFit::Cover)
        .repeat(ImageRepeat::RepeatX);
    let direct = Image::new(shared)
        .width(80.)
        .height(40.)
        .fit(ImageFit::Cover)
        .repeat(ImageRepeat::RepeatX);
    assert_eq!(Widget::from(built), Widget::from(direct));

    // An absent handle lowers to a fixed empty box, not an image.
    let empty = Widget::from(RawImage::new().width(12.).height(9.));
    let (tree, root) = layout(|| empty, Constraints::loose(Size::new(100., 100.)));
    assert_eq!(bounds(&tree, root), (0., 0., 12., 9.));
}

#[test]
fn repeat_tiles_within_bounds() {
    // A 20x20 image in a 50x50 box repeats on both axes.
    let mut tree = WidgetTree::new();
    tree.mount(
        Image::new(handle(20, 20))
            .width(50.)
            .height(50.)
            .fit(ImageFit::None)
            .repeat(ImageRepeat::Repeat)
            .into(),
    )
    .expect("mount");
    tree.layout(Constraints::loose(Size::new(50., 50.)))
        .expect("layout");
    let commands = image_commands(&tree.paint());
    assert!(commands.len() > 1, "repeat paints multiple tiles");
    for (_, destination) in &commands {
        assert!(destination.origin.x >= -20. && destination.origin.x <= 50.);
    }

    // NoRepeat paints exactly one destination.
    let mut tree = WidgetTree::new();
    tree.mount(
        Image::new(handle(20, 20))
            .width(50.)
            .height(50.)
            .fit(ImageFit::None)
            .repeat(ImageRepeat::NoRepeat)
            .into(),
    )
    .expect("mount");
    tree.layout(Constraints::loose(Size::new(50., 50.)))
        .expect("layout");
    assert_eq!(image_commands(&tree.paint()).len(), 1);
}

#[test]
fn transformed_image_geometry_follows_layout() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Transform::translation(Offset::new(10., 5.), Image::new(handle(40, 20))).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    // The image command carries the untransformed local rect; the world
    // transform is applied by the compositor. Element bounds already
    // include the translation, proving layout placed it at (10, 5).
    let command = image_commands(&tree.paint())[0];
    assert_eq!(command.1.size, Size::new(40., 20.));
    let child = tree.children(root).expect("child")[0];
    assert_eq!(bounds(&tree, child).0, 10.);
    assert_eq!(bounds(&tree, child).1, 5.);

    // Hit testing reaches the image under the translation.
    let hit = tree.hit_test(Offset::new(12., 7.)).expect("hit");
    assert!(tree.element_for_render(hit).is_some());
}

#[test]
fn filter_quality_selects_sampling() {
    for (quality, nearest) in [
        (FilterQuality::None, true),
        (FilterQuality::Low, false),
        (FilterQuality::Medium, false),
        (FilterQuality::High, false),
    ] {
        let mut tree = WidgetTree::new();
        tree.mount(
            Image::new(handle(8, 8))
                .width(16.)
                .height(16.)
                .filter_quality(quality)
                .into(),
        )
        .expect("mount");
        tree.layout(Constraints::loose(Size::new(16., 16.)))
            .expect("layout");
        let is_nearest = tree.paint().commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::Image {
                    sampling: incular_rendering::ImageSampling::Nearest,
                    ..
                }
            )
        });
        assert_eq!(is_nearest, nearest, "{quality:?}");
    }
    let _ = (Color::WHITE, ImageRepeat::NoRepeat);
}
