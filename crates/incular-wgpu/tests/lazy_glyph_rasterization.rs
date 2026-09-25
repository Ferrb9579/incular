//! Keep native glyph placement and coverage correct when changing rasterizers.
use incular_text::{TextAlign, TextEngine, TextStyle};
use incular_wgpu::GlyphAtlas;
use std::collections::HashSet;

#[test]
fn fractional_dpi_size_does_not_alias_the_next_whole_pixel_size() {
    let mut text = TextEngine::new();
    let layout = text.layout(
        "H",
        &TextStyle::new().font_family("Arial").font_size(13.),
        None,
        TextAlign::Start,
    );
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();
    let fractional = atlas
        .lookup_or_rasterize(run, glyph, 1.5, &HashSet::new())
        .unwrap();
    let mut whole_run = (**run).clone();
    whole_run.font_size = 20.;
    let whole = atlas
        .lookup_or_rasterize(&whole_run, glyph, 1., &HashSet::new())
        .unwrap();
    assert_ne!(
        fractional.entry, whole.entry,
        "19.5 ppem must not reuse the 20 ppem mask"
    );
    let diagnostic = atlas.debug_glyph(run, glyph, 1.5).unwrap();
    assert_eq!(diagnostic.requested_physical_size, 19);
    assert_eq!(diagnostic.requested_fractional_size, 32);
    let mask = fractional.bitmap.unwrap();
    let actual_area: f64 = mask.iter().map(|v| f64::from(*v) / 255.).sum();
    let reference = fontdue::Font::from_bytes(
        run.font.bytes().as_ref(),
        fontdue::FontSettings {
            collection_index: run.font.face_index(),
            ..Default::default()
        },
    )
    .unwrap();
    let (_, expected) = reference.rasterize_indexed(glyph, 19.5);
    let expected_area: f64 = expected.iter().map(|v| f64::from(*v) / 255.).sum();
    assert!(
        (actual_area / expected_area - 1.).abs() < 0.02,
        "fractional em scale changed the glyph's ink area"
    );
}

#[test]
fn fractional_origins_move_ink_without_changing_area_and_warm_the_bounded_cache() {
    let mut text = TextEngine::new();
    let layout = text.layout(
        "Ag",
        &TextStyle::new().font_family("Arial").font_size(19.5),
        None,
        TextAlign::Start,
    );
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();
    let mut original = None;
    let mut entries = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            let raster = atlas
                .lookup_or_rasterize_at(run, glyph, 1., [x, y], &HashSet::new())
                .unwrap();
            let entry = raster.entry;
            let mut area = 0.;
            let mut center = [0., 0.];
            for (index, value) in raster.bitmap.unwrap().iter().enumerate() {
                let coverage = f64::from(*value) / 255.;
                let px = index % usize::from(entry.width);
                let py = index / usize::from(entry.width);
                area += coverage;
                center[0] += (px as f64 + f64::from(entry.bearing_x) + 0.5) * coverage;
                center[1] += (py as f64 - f64::from(entry.bearing_y) - f64::from(entry.height)
                    + 0.5)
                    * coverage;
            }
            center = center.map(|v| v / area);
            let (original_area, original_center) = *original.get_or_insert((area, center));
            assert!((area / original_area - 1.).abs() < 0.01);
            assert!((center[0] - original_center[0] - f64::from(x) / 4.).abs() < 0.08);
            assert!((center[1] - original_center[1] - f64::from(y) / 4.).abs() < 0.08);
            entries.push(entry);
        }
    }
    for y in 0..4 {
        for x in 0..4 {
            let hit = atlas
                .lookup_or_rasterize_at(run, glyph, 1., [x, y], &HashSet::new())
                .unwrap();
            assert!(hit.bitmap.is_none());
            assert_eq!(hit.entry, entries[usize::from(y * 4 + x)]);
        }
    }
    assert_eq!(atlas.counters().glyphs_rasterized, 16);
    assert_eq!(atlas.counters().glyph_cache_hits, 16);
    assert_eq!(atlas.counters().font_parser_cache_misses, 1);
    atlas.set_max_pages(0, &HashSet::new());
    assert!(
        atlas
            .lookup_or_rasterize_at(run, glyph, 1., [1, 1], &HashSet::new())
            .is_none()
    );
}

#[test]
fn on_demand_masks_preserve_em_scale_baseline_and_antialiased_coverage() {
    let mut text = TextEngine::new();
    let layout = text.layout(
        "AgjQÉÅ",
        &TextStyle::new().font_family("Arial").font_size(24.0),
        None,
        TextAlign::Start,
    );
    let mut atlas = GlyphAtlas::new();
    for run in layout.lines.iter().flat_map(|line| line.runs.iter()) {
        // Independent, previous production rasterizer. Compare masks in common
        // baseline coordinates, allowing one-pixel conservative outline bounds.
        let reference = fontdue::Font::from_bytes(
            run.font.bytes().as_ref(),
            fontdue::FontSettings {
                collection_index: run.font.face_index(),
                ..Default::default()
            },
        )
        .unwrap();
        for scale in [1.0, 1.5, 2.0] {
            for glyph in run.glyphs.iter() {
                let actual = atlas
                    .lookup_or_rasterize(run, glyph.id, scale, &HashSet::new())
                    .unwrap();
                let bitmap = actual.bitmap.unwrap();
                let entry = actual.entry;
                let (metrics, expected) =
                    reference.rasterize_indexed(glyph.id, (24.0 * scale) as f32);
                let ax = i32::from(entry.bearing_x);
                let ay = -i32::from(entry.bearing_y) - i32::from(entry.height);
                let ey = -metrics.ymin - metrics.height as i32;
                assert!((ax - metrics.xmin).abs() <= 1);
                assert!((ay - ey).abs() <= 1);
                assert!((i32::from(entry.width) - metrics.width as i32).abs() <= 2);
                assert!((i32::from(entry.height) - metrics.height as i32).abs() <= 2);
                let mut difference = 0_u64;
                let mut area = 0_u64;
                for y in ay.min(ey)..(ay + i32::from(entry.height)).max(ey + metrics.height as i32)
                {
                    for x in ax.min(metrics.xmin)
                        ..(ax + i32::from(entry.width)).max(metrics.xmin + metrics.width as i32)
                    {
                        let a = pixel(
                            &bitmap,
                            usize::from(entry.width),
                            usize::from(entry.height),
                            x - ax,
                            y - ay,
                        );
                        let b = pixel(
                            &expected,
                            metrics.width,
                            metrics.height,
                            x - metrics.xmin,
                            y - ey,
                        );
                        difference += u64::from(a.abs_diff(b));
                        area += 1;
                    }
                }
                assert!(
                    difference as f64 / ((area * 255) as f64) < 0.06,
                    "coverage changed at {scale}x for glyph {}",
                    glyph.id
                );
                assert!(bitmap.iter().any(|value| *value > 0 && *value < 255));
            }
        }
    }
}

fn pixel(bytes: &[u8], width: usize, height: usize, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
        0
    } else {
        bytes[y as usize * width + x as usize]
    }
}
