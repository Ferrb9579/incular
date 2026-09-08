use std::collections::HashSet;

use incular_assets::FontId;
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::{
    BlendMode, DisplayList, FillRule, GaussianBlur, LayerTree, PaintCommand, Path, Stroke,
};
use incular_text::{TextAlign, TextEngine, TextStyle};
use incular_wgpu::*;

#[test]
fn public_pipeline_inputs_preserve_painter_order() {
    let mut list = DisplayList::new();
    list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 4.)),
        color: Color::WHITE,
    });
    list.push(PaintCommand::PushTransform {
        transform: incular_core::Transform::translation(Offset::new(8., 0.)),
    });
    list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 4.)),
        color: Color::BLACK,
    });
    list.push(PaintCommand::PopTransform);

    let plan = BatchPlan::lower(&list);
    assert_eq!(plan.rectangle_count(), 2);
    assert_eq!(plan.batches()[0].instances()[0].color, Color::WHITE);
    assert_eq!(
        plan.batches()[0].instances()[1].rect.origin,
        Offset::new(8., 0.)
    );
}

#[test]
fn public_path_contract_accepts_fill_and_stroke_geometry() {
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(0., 0.))
        .line_to(Offset::new(20., 0.))
        .line_to(Offset::new(10., 16.))
        .close();
    let path = builder.build();
    let fill = tessellate_path(&path, FillRule::NonZero, None).expect("fill mesh");
    let stroke = tessellate_path(
        &path,
        FillRule::NonZero,
        Some(Stroke {
            width: 2.,
            ..Stroke::default()
        }),
    )
    .expect("stroke mesh");
    assert!(!fill.indices.is_empty());
    assert!(!stroke.indices.is_empty());
    assert!(
        fill.vertices
            .iter()
            .chain(stroke.vertices.iter())
            .flatten()
            .all(|value| value.is_finite())
    );
}

#[test]
fn public_blend_contracts_are_unique_and_destination_reads_are_explicit() {
    let modes = [
        BlendMode::SrcOver,
        BlendMode::Src,
        BlendMode::DstOver,
        BlendMode::SrcIn,
        BlendMode::DstIn,
        BlendMode::SrcOut,
        BlendMode::DstOut,
        BlendMode::SrcAtop,
        BlendMode::DstAtop,
        BlendMode::Xor,
        BlendMode::Plus,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
    ];
    let mut codes = modes.iter().map(|mode| mode.code()).collect::<Vec<_>>();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), modes.len());
    assert!(!BlendMode::SrcOver.requires_destination_read());
    assert!(!BlendMode::Src.requires_destination_read());
    assert!(BlendMode::Multiply.requires_destination_read());
    assert!(BlendMode::Exclusion.requires_destination_read());
}

#[test]
fn public_effect_contracts_emit_balanced_isolated_commands() {
    let mut tree = LayerTree::new();
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(20., 30.), Size::new(40., 50.)),
    );
    let blur = tree.create_blur(GaussianBlur::uniform(4.));
    tree.set_children(blur, vec![picture]);
    tree.set_root(blur);

    let list = tree.flatten();
    assert!(matches!(
        list.commands().first(),
        Some(PaintCommand::PushBlur { bounds, .. })
            if *bounds == Rect::from_origin_size(Offset::new(20., 30.), Size::new(40., 50.))
    ));
    assert!(matches!(
        list.commands().last(),
        Some(PaintCommand::PopEffect)
    ));
    assert_eq!(
        list.commands()
            .iter()
            .filter(|command| matches!(command, PaintCommand::PushBlur { .. }))
            .count(),
        1
    );
}

#[test]
fn public_glyph_pipeline_contract_keeps_cache_identity_and_dpi_variants() {
    let mut text = TextEngine::new();
    let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();

    let first = atlas
        .lookup_or_rasterize(run, glyph, 1., &HashSet::new())
        .unwrap();
    let warm = atlas
        .lookup_or_rasterize(run, glyph, 1., &HashSet::new())
        .unwrap();
    let high_dpi = atlas
        .lookup_or_rasterize(run, glyph, 2., &HashSet::new())
        .unwrap();
    assert_eq!(first.entry, warm.entry);
    assert_ne!(first.entry, high_dpi.entry);
    assert_eq!(atlas.counters().glyph_cache_hits, 1);
    assert!(
        atlas
            .entry(GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size: GlyphRasterRequest::new(run.font_size, 2.).physical_size,
            })
            .is_some()
    );
}

#[test]
fn public_resource_identity_contract_separates_images_and_glyph_sizes() {
    let mut registry = SharedGpuResourceRegistry::default();
    let image = registry.image_identity(incular_image::ImageId(7));
    assert_eq!(registry.image_identity(incular_image::ImageId(7)), image);

    let glyph = GlyphCacheKey {
        font: FontId(2),
        glyph: 65,
        physical_size: 16,
    };
    let one_x = registry.glyph_identity(glyph);
    let two_x = registry.glyph_identity(GlyphCacheKey {
        physical_size: 32,
        ..glyph
    });
    assert_ne!(one_x, two_x);
    assert_eq!(registry.image_count(), 1);
    assert_eq!(registry.glyph_count(), 2);
}
