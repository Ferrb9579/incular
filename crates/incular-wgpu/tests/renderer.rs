use incular_assets::{FontHandle, FontId};
use incular_core::{Color, Offset, Rect, Size, Transform};
use incular_image::{ImageHandle, ImageId};
use incular_platform::PhysicalSize;
use incular_rendering::{
    BlendMode, DisplayList, DropShadowEffect, FillRule, GaussianBlur, ImageSampling, LayerTree,
    LineCap, LineJoin, PaintCommand, Path, Stroke, gaussian_kernel_weights, normalize_opacity,
    normalize_sigma, sample_gradient_stops,
};
use incular_text::{TextAlign, TextEngine, TextStyle};
use incular_wgpu::*;

#[test]
fn gradient_sampling_contains_five_stop_middle_colors() {
    let stops = incular_rendering::GradientStops::new(vec![
        incular_rendering::GradientStop {
            offset: 0.,
            color: Color::rgba(255, 0, 0, 255),
        },
        incular_rendering::GradientStop {
            offset: 0.25,
            color: Color::rgba(255, 255, 0, 255),
        },
        incular_rendering::GradientStop {
            offset: 0.5,
            color: Color::rgba(0, 255, 0, 255),
        },
        incular_rendering::GradientStop {
            offset: 0.75,
            color: Color::rgba(0, 255, 255, 255),
        },
        incular_rendering::GradientStop {
            offset: 1.,
            color: Color::rgba(0, 0, 255, 255),
        },
    ]);
    let red = sample_gradient_stops(&stops, 0.);
    let yellow = sample_gradient_stops(&stops, 0.25);
    let green = sample_gradient_stops(&stops, 0.5);
    let cyan = sample_gradient_stops(&stops, 0.75);
    let blue = sample_gradient_stops(&stops, 1.);
    assert_eq!(red, [1., 0., 0., 1.]);
    assert!(yellow[0] > 0.99 && yellow[1] > 0.99);
    assert!(green[1] > 0.99 && green[0] < 0.01);
    assert!(cyan[1] > 0.99 && cyan[2] > 0.99);
    assert_eq!(blue, [0., 0., 1., 1.]);
}

#[test]
fn lyon_tessellates_curves_and_fill_rules() {
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(0., 0.))
        .quadratic_to(Offset::new(20., 30.), Offset::new(40., 0.))
        .cubic_to(
            Offset::new(35., 20.),
            Offset::new(5., 20.),
            Offset::new(0., 0.),
        )
        .close();
    let path = builder.build();
    for rule in [FillRule::NonZero, FillRule::EvenOdd] {
        let mesh = tessellate_path(&path, rule, None).expect("finite curve mesh");
        assert!(!mesh.indices.is_empty());
        assert!(
            mesh.vertices
                .iter()
                .all(|point| point[0].is_finite() && point[1].is_finite())
        );
    }
}

#[test]
fn stroke_tessellation_maps_caps_joins_and_widths_to_distinct_meshes() {
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(1., 1.))
        .line_to(Offset::new(20., 1.))
        .line_to(Offset::new(20., 20.));
    let path = builder.build();
    let butt = Stroke {
        width: 2.,
        cap: LineCap::Butt,
        join: LineJoin::Miter,
        ..Stroke::default()
    };
    let round = Stroke {
        cap: LineCap::Round,
        join: LineJoin::Round,
        ..butt
    };
    let butt_mesh = tessellate_path(&path, FillRule::NonZero, Some(butt)).expect("butt stroke");
    let round_mesh = tessellate_path(&path, FillRule::NonZero, Some(round)).expect("round stroke");
    let wide_mesh = tessellate_path(&path, FillRule::NonZero, Some(Stroke { width: 6., ..butt }))
        .expect("wide stroke");
    assert_ne!(butt_mesh.indices, round_mesh.indices);
    assert_ne!(butt_mesh.vertices, wide_mesh.vertices);
}

#[test]
fn empty_and_degenerate_paths_are_safe_noop_meshes() {
    let empty = Path::default();
    assert!(
        tessellate_path(&empty, FillRule::NonZero, None).is_none_or(|mesh| mesh.indices.is_empty())
    );
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(3., 3.))
        .line_to(Offset::new(3., 3.));
    let degenerate = builder.build();
    assert!(
        tessellate_path(&degenerate, FillRule::NonZero, None)
            .is_none_or(|mesh| mesh.indices.is_empty())
    );
}

#[test]
fn lowering_preserves_order_and_transforms() {
    let mut list = DisplayList::new();
    list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
        color: Color::WHITE,
    });
    list.push(PaintCommand::PushTransform {
        transform: Transform::translation(Offset::new(2., 3.)),
    });
    list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
        color: Color::BLACK,
    });
    let plan = BatchPlan::lower(&list);
    assert_eq!(plan.rectangle_count(), 2);
    assert_eq!(
        plan.batches()[0].instances()[1].rect.origin,
        Offset::new(2., 3.)
    );
}

#[test]
fn affine_lowering_preserves_rotated_bounds() {
    let mut list = DisplayList::new();
    list.push(PaintCommand::PushTransform {
        transform: Transform::rotation(std::f32::consts::FRAC_PI_2),
    });
    list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(10., 20.)),
        color: Color::WHITE,
    });
    let plan = BatchPlan::lower(&list);
    let bounds = plan.batches()[0].instances()[0].rect;
    assert!((bounds.size.width - 20.).abs() < 0.0001);
    assert!((bounds.size.height - 10.).abs() < 0.0001);
}

#[test]
fn rectangle_batches_expose_logical_geometry() {
    let mut list = DisplayList::new();
    list.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::new(10., 10.), Size::new(20., 10.)),
        color: Color::WHITE,
    });
    let plan = BatchPlan::lower(&list);
    assert_eq!(
        plan.batches()[0].instances()[0].rect.origin,
        Offset::new(10., 10.)
    );
    assert_eq!(
        plan.batches()[0].instances()[0].rect.size,
        Size::new(20., 10.)
    );
}

#[test]
fn image_commands_preserve_top_to_bottom_source_regions() {
    let image = ImageHandle::from_rgba8(
        2,
        2,
        [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 128,
        ],
    )
    .unwrap();
    let destination = Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.));
    let mut list = DisplayList::new();
    list.push(PaintCommand::Image {
        image,
        source: Rect::from_origin_size(Offset::new(1., 0.), Size::new(1., 2.)),
        destination,
        sampling: ImageSampling::Linear,
    });
    let PaintCommand::Image { source, .. } = &list.commands()[0] else {
        unreachable!("image command missing")
    };
    assert_eq!(
        *source,
        Rect::from_origin_size(Offset::new(1., 0.), Size::new(1., 2.))
    );
}

#[test]
fn image_sampling_has_only_retained_renderer_neutral_modes() {
    assert_ne!(ImageSampling::Linear, ImageSampling::Nearest);
    assert_eq!(ImageSampling::default(), ImageSampling::Linear);
}

#[test]
fn atlas_starts_with_one_normal_page() {
    let atlas = GlyphAtlas::new();
    let memory = atlas.memory();
    assert_eq!(memory.normal_pages, 1);
    assert_eq!(memory.oversize_pages, 0);
    assert_eq!(atlas.counters().glyph_atlas_pages, 1);
}

#[test]
fn atlas_uvs_use_the_allocated_region() {
    let entry = AtlasEntry {
        page: 3,
        x: 11,
        y: 20,
        width: 10,
        height: 4,
        atlas_class: GlyphAtlasClass::Normal,
        bearing_x: -2,
        bearing_y: 3,
    };
    assert_eq!(
        entry.uv_rect(),
        [11. / 1024., 20. / 1024., 21. / 1024., 24. / 1024.]
    );
    assert_eq!(entry.allocation_rect(), [10, 19, 12, 6]);
}

#[test]
fn physical_raster_requests_cover_supported_dpi_scales_once() {
    for (scale, physical) in [
        (1.0, 16),
        (1.25, 20),
        (1.5, 24),
        (1.75, 28),
        (2.0, 32),
        (2.5, 40),
        (3.0, 48),
    ] {
        let request = GlyphRasterRequest::new(16., scale);
        assert_eq!(request.physical_size, physical);
        assert_eq!(request.logical_font_size, 16.);
        assert_eq!(request.scale_factor, scale as f32);
        assert!(request.supported);
    }
}

#[test]
fn glyph_atlas_exposes_linear_coverage_filtering_and_padding() {
    assert_eq!(ImageSampling::default(), ImageSampling::Linear);
    assert_eq!(GLYPH_ATLAS_PADDING, 1);
}

#[test]
fn atlas_padding_keeps_odd_sized_content_inside_allocation() {
    let entry = AtlasEntry {
        page: 0,
        x: GLYPH_ATLAS_PADDING,
        y: GLYPH_ATLAS_PADDING,
        width: 7,
        height: 9,
        atlas_class: GlyphAtlasClass::Normal,
        bearing_x: -1,
        bearing_y: 2,
    };
    assert_eq!(entry.allocation_rect(), [0, 0, 9, 11]);
    assert_eq!([entry.x, entry.y, entry.width, entry.height], [1, 1, 7, 9]);
}

#[test]
fn clips_intersect_in_logical_coordinates() {
    let mut tree = LayerTree::new();
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
    );
    let clip = tree.create_clip_rect(Rect::from_origin_size(
        Offset::new(0., 0.),
        Size::new(10., 10.),
    ));
    tree.set_children(clip, vec![picture]);
    tree.set_root(clip);
    let _ = tree.flatten();
    assert_eq!(
        tree.flattened_pictures()[0].active_clip,
        Some(Rect::from_origin_size(Offset::ZERO, Size::new(10., 10.)))
    );
}

#[test]
fn compositor_effects_keep_tight_scene_bounds_and_nested_boundaries() {
    let mut tree = LayerTree::new();
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(20., 30.), Size::new(40., 50.)),
    );
    let blur = tree.create_blur(GaussianBlur::uniform(4.));
    let shadow = tree.create_drop_shadow(DropShadowEffect::new(Offset::ZERO, 2., Color::BLACK));
    tree.set_children(blur, vec![picture]);
    tree.set_children(shadow, vec![blur]);
    tree.set_root(shadow);
    let list = tree.flatten();
    assert!(matches!(
        list.commands().first(),
        Some(PaintCommand::PushDropShadow { bounds, .. })
            if *bounds == Rect::from_origin_size(Offset::new(20., 30.), Size::new(40., 50.))
    ));
    assert!(
        list.commands()
            .iter()
            .any(|command| matches!(command, PaintCommand::PushBlur { .. }))
    );
    assert_eq!(
        list.commands()
            .iter()
            .filter(|command| matches!(command, PaintCommand::PopEffect))
            .count(),
        2
    );
}

#[test]
fn premultiplied_shadow_colorization_matches_source_over_inputs() {
    let color = Color::rgba(220, 40, 80, 128);
    let sample = incular_rendering::premultiplied_shadow_sample(color, 0.25);
    let [red, green, blue, alpha] = color.to_linear_rgba();
    let expected_alpha = alpha * 0.25;
    assert!((sample[3] - expected_alpha).abs() < 1e-6);
    assert!((sample[0] - red * expected_alpha).abs() < 1e-6);
    assert!((sample[1] - green * expected_alpha).abs() < 1e-6);
    assert!((sample[2] - blue * expected_alpha).abs() < 1e-6);
}

#[test]
fn premultiplied_blur_edge_preserves_color_alpha_ratio() {
    let weights = gaussian_kernel_weights(1.5);
    let center = weights.len() / 2;
    let mut output: [f32; 4] = [0.; 4];
    for (index, weight) in weights.iter().enumerate() {
        let sample = if index >= center {
            [1., 0., 0., 1.]
        } else {
            [0.; 4]
        };
        for channel in 0..4 {
            output[channel] += sample[channel] * weight;
        }
    }
    assert!(output[3] > 0. && output[3] < 1.);
    assert!((output[0] - output[3]).abs() < 1e-6);
    assert_eq!(output[1], 0.);
    assert_eq!(output[2], 0.);
}

#[test]
fn blur_normalization_and_kernel_support_are_public_contracts() {
    assert_eq!(normalize_sigma(f32::NAN), 0.);
    assert_eq!(normalize_sigma(-1.), 0.);
    assert_eq!(normalize_sigma(4.), 4.);
    assert_eq!(gaussian_kernel_weights(0.), vec![1.]);
    for sigma in [1., 16., 100.] {
        let kernel = gaussian_kernel_weights(sigma);
        assert!(!kernel.is_empty());
        assert!((kernel.iter().sum::<f32>() - 1.).abs() < 1e-5);
        assert!(kernel.iter().all(|weight| weight.is_finite()));
    }
}

#[test]
fn raster_cache_reuses_color_independent_glyphs_but_not_dpi_size() {
    let mut text = TextEngine::new();
    let layout = text.layout("Hello", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();
    let first = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap().entry;
    let same = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap().entry;
    let higher_dpi = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap().entry;
    let one_x = GlyphRasterRequest::new(run.font_size, 1.0);
    let two_x = GlyphRasterRequest::new(run.font_size, 2.0);
    assert_eq!(first, same);
    assert_ne!(
        GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: one_x.physical_size,
        },
        GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: two_x.physical_size,
        }
    );
    assert!(
        atlas
            .entry(GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size: two_x.physical_size,
            })
            .is_some()
    );
    assert!(higher_dpi.width > 0);
    assert_eq!(atlas.counters().glyph_cache_hits, 1);
}

#[test]
fn dpi_change_creates_one_new_variant_then_warms() {
    let mut text = TextEngine::new();
    let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();
    let _ = atlas.lookup_or_rasterize(run, glyph, 1.0).unwrap();
    let one_x = atlas.counters();
    let _ = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap();
    let two_x = atlas.counters();
    let _ = atlas.lookup_or_rasterize(run, glyph, 2.0).unwrap();
    let warm_two_x = atlas.counters();
    assert_eq!(two_x.glyphs_rasterized - one_x.glyphs_rasterized, 1);
    assert_eq!(two_x.glyph_atlas_uploads - one_x.glyph_atlas_uploads, 1);
    assert_eq!(warm_two_x.glyphs_rasterized, two_x.glyphs_rasterized);
    assert_eq!(warm_two_x.glyph_atlas_uploads, two_x.glyph_atlas_uploads);
}

#[test]
fn glyph_debug_reports_logical_and_physical_units() {
    let mut text = TextEngine::new();
    let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();
    let _ = atlas.lookup_or_rasterize(run, glyph, 1.5).unwrap();
    let info = atlas
        .debug_glyph(run, glyph, 1.5)
        .expect("cached diagnostic");
    assert_eq!(info.logical_font_size, run.font_size);
    assert_eq!(
        info.requested_physical_size,
        (run.font_size * 1.5).round() as u16
    );
    assert_eq!(
        info.allocation_rect[2],
        info.bitmap_size[0] + 2 * GLYPH_ATLAS_PADDING
    );
    assert_eq!(
        info.allocation_rect[3],
        info.bitmap_size[1] + 2 * GLYPH_ATLAS_PADDING
    );
    assert_eq!(
        info.bitmap_bytes,
        usize::from(info.bitmap_size[0]) * usize::from(info.bitmap_size[1])
    );
}

#[test]
fn raster_debug_classifies_physical_ppem() {
    let mut text = TextEngine::new();
    let mut atlas = GlyphAtlas::new();
    for (size, scale, class) in [
        (6., 1., GlyphAtlasClass::Normal),
        (16., 1., GlyphAtlasClass::Normal),
        (48., 2., GlyphAtlasClass::Normal),
    ] {
        let layout = text.layout(
            "H",
            &TextStyle {
                size,
                ..TextStyle::default()
            },
            None,
            TextAlign::Start,
        );
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        atlas
            .lookup_or_rasterize(run, glyph, scale)
            .expect("supported size");
        assert_eq!(
            atlas.debug_glyph(run, glyph, scale).unwrap().atlas_class,
            class
        );
    }
}

#[test]
fn glyph_masks_are_safe_and_warm_across_supported_sizes() {
    let mut text = TextEngine::new();
    let mut atlas = GlyphAtlas::new();
    for size in [4., 8., 16., 48., 96., 256., 512., 1024.] {
        let layout = text.layout(
            "H",
            &TextStyle {
                size,
                ..TextStyle::default()
            },
            None,
            TextAlign::Start,
        );
        let run = &layout.lines[0].runs[0];
        let glyph = run.glyphs[0].id;
        let first = atlas
            .lookup_or_rasterize(run, glyph, 1.)
            .expect("supported size");
        assert!(first.entry.width > 0 && first.entry.height > 0);
        let debug = atlas.debug_glyph(run, glyph, 1.).unwrap();
        assert_eq!(
            debug.bitmap_bytes,
            first.entry.width as usize * first.entry.height as usize
        );
        assert!(atlas.lookup_or_rasterize(run, glyph, 1.).is_some());
    }
    assert!(atlas.counters().micro_glyph_rasters > 0);
    assert!(atlas.counters().normal_glyph_rasters > 0);
    assert!(atlas.counters().huge_glyph_rasters > 0);
}

#[test]
fn unsupported_gigantic_requests_are_rejected_without_rasterizing() {
    let request = GlyphRasterRequest::new(1_000_000_000., 1.);
    assert!(!request.supported);
    let mut text = TextEngine::new();
    let layout = text.layout(
        "H",
        &TextStyle {
            size: 1_000_000_000.,
            ..TextStyle::default()
        },
        None,
        TextAlign::Start,
    );
    let run = &layout.lines[0].runs[0];
    let mut atlas = GlyphAtlas::new();
    assert!(
        atlas
            .lookup_or_rasterize(run, run.glyphs[0].id, 1.)
            .is_none()
    );
    assert_eq!(atlas.counters().glyphs_rasterized, 0);
}

#[test]
fn fractional_gpu_placement_reuses_one_fontdue_mask() {
    let mut text = TextEngine::new();
    let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut atlas = GlyphAtlas::new();
    let first = atlas.lookup_or_rasterize(run, glyph, 1.).unwrap().entry;
    let cold = atlas.counters();
    for _x in [0., 0.25, 0.5, 0.75] {
        assert_eq!(
            atlas.lookup_or_rasterize(run, glyph, 1.).unwrap().entry,
            first
        );
    }
    let warm = atlas.counters();
    assert_eq!(warm.glyphs_rasterized, cold.glyphs_rasterized);
    assert_eq!(warm.glyph_atlas_uploads, cold.glyph_atlas_uploads);
}

#[test]
fn glyph_cache_keeps_font_ids_separate() {
    let mut text = TextEngine::new();
    let layout = text.layout("H", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    let mut alternate = (**run).clone();
    alternate.font = FontHandle::with_face_index(
        FontId(run.font.id().0.wrapping_add(1)),
        run.font.bytes().clone(),
        run.font.face_index(),
    );

    let mut atlas = GlyphAtlas::new();
    let first = atlas.lookup_or_rasterize(run, glyph, 1.).unwrap().entry;
    let before_alternate = atlas.counters();
    let second = atlas
        .lookup_or_rasterize(&alternate, glyph, 1.)
        .unwrap()
        .entry;
    let after_alternate = atlas.counters();
    let physical_size = GlyphRasterRequest::new(run.font_size, 1.).physical_size;
    assert!(
        atlas
            .entry(GlyphCacheKey {
                font: run.font.id(),
                glyph,
                physical_size,
            })
            .is_some()
    );
    assert!(
        atlas
            .entry(GlyphCacheKey {
                font: alternate.font.id(),
                glyph,
                physical_size,
            })
            .is_some()
    );
    assert_ne!(first, second);
    assert_eq!(
        after_alternate.glyphs_rasterized - before_alternate.glyphs_rasterized,
        1
    );
    assert_eq!(
        after_alternate.font_parser_cache_misses - before_alternate.font_parser_cache_misses,
        1
    );
}

#[test]
fn counter_text_warms_the_atlas_incrementally() {
    fn rasterize(atlas: &mut GlyphAtlas, text: &mut TextEngine, value: &str) {
        for label in ["Incular Counter", value, "Increment"] {
            let layout = text.layout(label, &TextStyle::default(), None, TextAlign::Start);
            for line in layout.lines.iter() {
                for run in line.runs.iter() {
                    for glyph in run.glyphs.iter() {
                        let _ = atlas.lookup_or_rasterize(run, glyph.id, 1.0);
                    }
                }
            }
        }
    }
    let mut atlas = GlyphAtlas::new();
    let mut text = TextEngine::new();
    rasterize(&mut atlas, &mut text, "Count: 0");
    let first = atlas.counters();
    rasterize(&mut atlas, &mut text, "Count: 1");
    let first_click = atlas.counters();
    rasterize(&mut atlas, &mut text, "Count: 1");
    let warm = atlas.counters();
    assert!(first.glyphs_rasterized > 0 && first.glyph_atlas_uploads > 0);
    assert_eq!(first_click.glyph_cache_misses - first.glyph_cache_misses, 1);
    assert_eq!(
        first_click.glyph_atlas_uploads - first.glyph_atlas_uploads,
        1
    );
    assert_eq!(warm.glyph_cache_misses, first_click.glyph_cache_misses);
    assert_eq!(warm.glyphs_rasterized, first_click.glyphs_rasterized);
    assert_eq!(warm.glyph_atlas_uploads, first_click.glyph_atlas_uploads);
}

fn premultiplied_over(dst: [f32; 4], src: [f32; 4]) -> [f32; 4] {
    incular_rendering::blend_premultiplied(BlendMode::SrcOver, src, dst)
}

fn apply_group_alpha(sample: [f32; 4], alpha: f32) -> [f32; 4] {
    let alpha = normalize_opacity(alpha);
    sample.map(|channel| channel * alpha)
}

#[test]
fn isolated_group_alpha_is_applied_once_to_overlap() {
    let red = [1., 0., 0., 1.];
    let blue = [0., 0., 1., 1.];
    let group_overlap = premultiplied_over(red, blue);
    let isolated = apply_group_alpha(group_overlap, 0.5);
    assert_eq!(isolated, [0., 0., 0.5, 0.5]);
    let descendant_alpha =
        premultiplied_over(apply_group_alpha(red, 0.5), apply_group_alpha(blue, 0.5));
    assert!((descendant_alpha[3] - 0.75).abs() < f32::EPSILON);
    assert_ne!(isolated, descendant_alpha);
}

#[test]
fn premultiplied_partial_child_and_nested_opacity_are_stable() {
    let child = [0.5, 0., 0., 0.5];
    assert_eq!(apply_group_alpha(child, 0.5), [0.25, 0., 0., 0.25]);
    let nested = apply_group_alpha(apply_group_alpha([0.2, 0.4, 0.6, 1.], 0.5), 0.5);
    assert_eq!(nested, [0.05, 0.1, 0.15, 0.25]);
}

#[test]
fn blend_codes_are_unique_and_reference_math_is_finite() {
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
    for mode in modes {
        for (source, destination) in [
            ([0., 0., 0., 0.], [0., 0., 0., 0.]),
            ([0.2, 0.1, 0.05, 0.5], [0.3, 0.2, 0.1, 0.5]),
            ([0.1, 0.3, 0.2, 1.], [0.4, 0.1, 0.7, 1.]),
        ] {
            let output = incular_rendering::blend_premultiplied(mode, source, destination);
            assert!(output.iter().all(|value| value.is_finite()));
            assert!(output[..3].iter().all(|value| *value <= output[3] + 1e-6));
        }
    }
}

#[test]
fn destination_blend_classification_keeps_porter_duff_on_fixed_path() {
    assert!(!BlendMode::SrcOver.requires_destination_read());
    assert!(!BlendMode::Src.requires_destination_read());
    assert!(!BlendMode::Plus.requires_destination_read());
    assert!(BlendMode::Multiply.requires_destination_read());
    assert!(BlendMode::Overlay.requires_destination_read());
    assert!(BlendMode::Exclusion.requires_destination_read());
}

#[test]
fn shared_resource_registry_reuses_image_and_matching_glyph_identity() {
    let mut resources = SharedGpuResourceRegistry::default();
    let image = ImageId(17);
    let image_a = resources.image_identity(image);
    let image_b = resources.image_identity(image);
    assert_eq!(image_a, image_b);

    let one_x = GlyphCacheKey {
        font: FontId(4),
        glyph: 73,
        physical_size: 16,
    };
    let same_one_x = resources.glyph_identity(one_x);
    assert_eq!(resources.glyph_identity(one_x), same_one_x);
    assert_ne!(
        resources.glyph_identity(GlyphCacheKey {
            physical_size: 32,
            ..one_x
        }),
        same_one_x
    );
    assert_eq!(resources.image_count(), 1);
    assert_eq!(resources.glyph_count(), 2);
}

#[test]
fn per_window_presentation_resize_loss_and_zero_size_are_isolated() {
    let mut window_a = WindowGpuPresentation::new(PhysicalSize::new(640, 480));
    let window_b = WindowGpuPresentation::new(PhysicalSize::new(1920, 1080));
    let b_before = window_b;

    assert!(window_a.resize(PhysicalSize::new(800, 600)));
    let after_resize = window_a.surface_generation;
    window_a.surface_lost();
    assert!(window_a.surface_generation > after_resize);
    assert_eq!(window_b, b_before);

    assert!(!window_a.resize(PhysicalSize::new(0, 600)));
    assert!(!window_a.configured);
    assert_eq!(window_a.physical_size, PhysicalSize::new(0, 600));
    assert_eq!(window_b, b_before);
    window_a.record_present();
    assert_eq!(window_a.presented_frames, 1);
    assert_eq!(window_b.presented_frames, 0);
}
