use incular_core::{Color, Lerp, Offset, Rect, Size, Transform};
use incular_rendering::*;
use std::sync::Arc;

#[test]
fn commands_keep_painter_order() {
    let mut c = Canvas::default();
    c.rect(
        Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
        Color::WHITE,
    );
    assert!(matches!(
        c.finish().commands()[0],
        PaintCommand::Rect { .. }
    ));
}

#[test]
fn filter_quality_maps_to_sampler() {
    assert_eq!(FilterQuality::None.sampling(), ImageSampling::Nearest);
    assert_eq!(FilterQuality::Low.sampling(), ImageSampling::Linear);
    assert_eq!(
        ImageSampling::from(FilterQuality::Medium),
        ImageSampling::Linear
    );
    assert_eq!(
        ImageSampling::from(FilterQuality::High),
        ImageSampling::Linear
    );
}

#[test]
fn paint_lowers_to_existing_display_commands() {
    let mut builder = Path::builder();
    builder.move_to(Offset::ZERO).line_to(Offset::new(10., 0.));
    let path = Arc::new(builder.build());

    let mut canvas = Canvas::default();
    canvas.draw_path(
        path.clone(),
        &Paint::new().color(Color::WHITE),
        FillRule::NonZero,
    );
    canvas.draw_path(
        path,
        &Paint::new().stroke().stroke_width(2.),
        FillRule::NonZero,
    );
    assert!(matches!(
        canvas.finish().commands(),
        [PaintCommand::FillPath { .. }, PaintCommand::StrokePath { stroke, .. }]
            if stroke.width == 2.
    ));
}

#[test]
fn shadow_lowers_correctly() {
    let first = Shadow::new(Color::BLACK, Offset::ZERO, -2.);
    let second = Shadow::new(Color::WHITE, Offset::new(10., 4.), 6.);
    assert_eq!(first.blur_radius, 0.);
    let midpoint = first.lerp(&second, 0.5);
    assert_eq!(midpoint.offset, Offset::new(5., 2.));
    assert_eq!(midpoint.blur_radius, 3.);
}

#[test]
fn scene_bounds_include_transforms_clips_and_effect_support() {
    let mut tree = LayerTree::new();
    let root = tree.create_transform(Transform::translation(Offset::new(5., 7.)));
    let shadow = tree.create_drop_shadow(DropShadowEffect::asymmetric(
        Offset::new(4., 3.),
        2.,
        1.,
        Color::BLACK,
    ));
    let clip = tree.create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.)));
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(2., 3.), Size::new(20., 10.)),
    );
    tree.set_children(root, vec![shadow]);
    tree.set_children(shadow, vec![clip]);
    tree.set_children(clip, vec![picture]);
    tree.set_root(root);

    // Source after transform: (7,10)..(27,20). Shadow blur expands the union
    // by 3*sigma around the +4,+3 translated shadow.
    assert_eq!(
        tree.scene_bounds(),
        Some(Rect::from_origin_size(
            Offset::new(5., 10.),
            Size::new(32., 16.)
        ))
    );
}
#[test]
fn radii_normalize_coherently_across_opposing_edges() {
    let r = CornerRadii {
        top_left: 80.,
        top_right: 80.,
        bottom_right: 20.,
        bottom_left: 20.,
    }
    .normalized(Size::new(100., 60.));
    assert!(r.top_left + r.top_right <= 100.);
    assert_eq!(r.top_left + r.bottom_left, 60.);
}
#[test]
fn gradient_stops_are_defined_for_empty_unsorted_and_single_inputs() {
    assert_eq!(GradientStops::new(Vec::new()).as_slice().len(), 2);
    let stops = GradientStops::new(vec![
        GradientStop {
            offset: 2.,
            color: Color::WHITE,
        },
        GradientStop {
            offset: -1.,
            color: Color::BLACK,
        },
    ]);
    assert_eq!(stops.as_slice()[0].offset, 0.);
    assert_eq!(stops.as_slice()[1].offset, 1.);
}
#[test]
fn path_bounds_are_tight_bezier_geometry_not_control_boxes() {
    let mut path = Path::builder();
    path.move_to(Offset::new(2., 3.)).cubic_to(
        Offset::new(-4., 8.),
        Offset::new(10., -2.),
        Offset::new(5., 6.),
    );
    let bounds = path.build().bounds().expect("curve has bounds");
    assert!(bounds.origin.x > -4. && bounds.origin.y > -2.);
    assert!(bounds.size.width < 14. && bounds.size.height < 10.);
}
#[test]
fn path_identity_is_stable_for_clones_and_unique_for_new_geometry() {
    let mut builder = Path::builder();
    builder.move_to(Offset::ZERO).line_to(Offset::new(1., 1.));
    let path = builder.build();
    assert_eq!(path.id(), path.clone().id());
    let mut replacement = Path::builder();
    replacement
        .move_to(Offset::ZERO)
        .line_to(Offset::new(1., 1.));
    assert_ne!(path.id(), replacement.build().id());
}
#[test]
fn retained_transform_reuses_picture_payload_and_accumulates_offsets() {
    let mut tree = LayerTree::new();
    let mut picture = DisplayList::new();
    picture.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(5., 5.)),
        color: Color::WHITE,
    });
    let leaf = tree.create_picture(
        picture,
        Rect::from_origin_size(Offset::ZERO, Size::new(5., 5.)),
    );
    let child = tree.create_transform(Transform::translation(Offset::new(5., 7.)));
    let parent = tree.create_transform(Transform::translation(Offset::new(10., 20.)));
    tree.set_children(child, vec![leaf]);
    tree.set_children(parent, vec![child]);
    tree.set_root(parent);
    let before = tree.diagnostics().picture_layers_repainted;
    let list = tree.flatten();
    assert_eq!(tree.diagnostics().picture_layers_repainted, before);
    assert!(list.commands().iter().any(|command| matches!(command, PaintCommand::PushTransform { transform } if transform.translation_offset() == Offset::new(15., 27.))));
    assert!(tree.update_transform(parent, Transform::translation(Offset::new(11., 20.))));
}
#[test]
fn nested_clips_cull_fully_outside_picture() {
    let mut tree = LayerTree::new();
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(20., 20.), Size::new(2., 2.)),
    );
    let clip = tree.create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::new(10., 10.)));
    tree.set_children(clip, vec![picture]);
    tree.set_root(clip);
    let _ = tree.flatten();
    assert_eq!(tree.diagnostics().layers_culled, 1);
}
#[test]
fn world_bounds_and_clips_use_the_same_accumulated_coordinates() {
    let mut tree = LayerTree::new();
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
    );
    let placement = tree.create_transform(Transform::translation(Offset::new(20., 30.)));
    let clip = tree.create_clip_rect(Rect::from_origin_size(
        Offset::new(15., 25.),
        Size::new(20., 20.),
    ));
    tree.set_children(clip, vec![placement]);
    tree.set_children(placement, vec![picture]);
    tree.set_root(clip);
    let _ = tree.flatten();
    assert_eq!(
        tree.flattened_pictures(),
        &[FlattenedPicture {
            layer: picture,
            local_bounds: Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
            world_bounds: Rect::from_origin_size(Offset::new(22., 33.), Size::new(10., 10.)),
            active_clip: Some(Rect::from_origin_size(
                Offset::new(15., 25.),
                Size::new(20., 20.),
            )),
        }]
    );
}

#[test]
fn opacity_normalization_is_finite_and_bounded() {
    assert_eq!(normalize_opacity(f32::NAN), 0.);
    assert_eq!(normalize_opacity(f32::NEG_INFINITY), 0.);
    assert_eq!(normalize_opacity(f32::INFINITY), 0.);
    assert_eq!(normalize_opacity(-1.), 0.);
    assert_eq!(normalize_opacity(2.), 1.);
    assert_eq!(normalize_opacity(0.35), 0.35);
}

#[test]
fn opacity_layer_isolated_group_is_retained_and_updates_composite_only() {
    let mut tree = LayerTree::new();
    let mut picture = DisplayList::new();
    picture.push(PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        color: Color::WHITE,
    });
    let leaf = tree.create_picture(
        picture,
        Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
    );
    let opacity = tree.create_opacity(0.5);
    tree.set_children(opacity, vec![leaf]);
    tree.set_root(opacity);
    let first = tree.flatten();
    assert!(matches!(
        first.commands().first(),
        Some(PaintCommand::PushOpacity { alpha, .. }) if *alpha == 0.5
    ));
    assert!(matches!(
        first.commands().last(),
        Some(PaintCommand::PopOpacity)
    ));
    let first_generation = match first.commands().first() {
        Some(PaintCommand::PushOpacity { generation, .. }) => *generation,
        _ => unreachable!("opacity command missing"),
    };
    let before = tree.diagnostics().opacity_updates;
    assert!(tree.update_opacity(opacity, 0.75));
    assert_eq!(tree.diagnostics().opacity_updates, before + 1);
    let second = tree.flatten();
    assert!(matches!(
        second.commands().first(),
        Some(PaintCommand::PushOpacity { alpha, .. }) if *alpha == 0.75
    ));
    let second_generation = match second.commands().first() {
        Some(PaintCommand::PushOpacity { generation, .. }) => *generation,
        _ => unreachable!("opacity command missing"),
    };
    assert_eq!(first_generation, second_generation);
}

#[test]
fn nested_opacity_alpha_invalidates_only_the_outer_content_key() {
    let mut tree = LayerTree::new();
    let leaf = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
    );
    let inner = tree.create_opacity(0.5);
    let outer = tree.create_opacity(0.5);
    tree.set_children(inner, vec![leaf]);
    tree.set_children(outer, vec![inner]);
    tree.set_root(outer);

    let first = tree.flatten();
    let generations = |list: &DisplayList| {
        list.commands()
            .iter()
            .filter_map(|command| match command {
                PaintCommand::PushOpacity {
                    layer, generation, ..
                } => Some((*layer, *generation)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let first_generations = generations(&first);
    assert_eq!(first_generations.len(), 2);

    assert!(tree.update_opacity(inner, 0.75));
    let second_generations = generations(&tree.flatten());
    assert_eq!(second_generations.len(), 2);
    assert_eq!(first_generations[1].0, second_generations[1].0);
    assert_eq!(first_generations[1].1, second_generations[1].1);
    assert_ne!(first_generations[0].1, second_generations[0].1);
}

#[test]
fn gaussian_kernel_is_normalized_and_symmetric() {
    for sigma in [0., 0.5, 2., 8., 32.] {
        let weights = gaussian_kernel_weights(sigma);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.).abs() < 1e-5, "sigma={sigma} sum={sum}");
        for (left, right) in weights.iter().zip(weights.iter().rev()) {
            assert!((left - right).abs() < 1e-6);
        }
    }
}

#[test]
fn effect_bounds_expand_outward_for_asymmetric_sigma_and_offsets() {
    let source = Rect::from_origin_size(Offset::new(10., 20.), Size::new(30., 40.));
    let blur = blur_bounds(source, 2., 4.);
    assert_eq!(blur.origin, Offset::new(4., 8.));
    assert_eq!(blur.size, Size::new(42., 64.));
    let shadow = drop_shadow_bounds(source, Offset::new(-9., 7.), 2., 4.);
    assert_eq!(shadow.origin, Offset::new(-5., 15.));
    assert_eq!(shadow.size, Size::new(45., 64.));
}

#[test]
fn effect_bounds_remain_conservative_for_fractional_dpi_equivalents() {
    let source = Rect::from_origin_size(Offset::new(0.25, 1.75), Size::new(12.5, 9.25));
    let logical_sigma = 2.25;
    let physical_sigma = logical_sigma * 1.5;
    let physical_margin = 3. * physical_sigma;
    let logical_margin = physical_margin / 1.5;
    assert_eq!(
        blur_bounds(source, logical_sigma, logical_sigma),
        Rect::from_origin_size(
            Offset::new(
                source.origin.x - logical_margin,
                source.origin.y - logical_margin
            ),
            Size::new(
                source.size.width + 2. * logical_margin,
                source.size.height + 2. * logical_margin,
            ),
        )
    );
}

#[test]
fn blur_and_shadow_layers_keep_source_generation_stable_for_parameter_updates() {
    let mut tree = LayerTree::new();
    let leaf = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
    );
    let blur = tree.create_blur(GaussianBlur::uniform(4.));
    tree.set_children(blur, vec![leaf]);
    tree.set_root(blur);
    let first = tree.flatten();
    let first_generation = match first.commands().first() {
        Some(PaintCommand::PushBlur { generation, .. }) => *generation,
        _ => panic!("blur command missing"),
    };
    assert!(tree.update_blur(blur, GaussianBlur::uniform(8.)));
    let second = tree.flatten();
    assert_eq!(
        first_generation,
        match second.commands().first() {
            Some(PaintCommand::PushBlur { generation, .. }) => *generation,
            _ => panic!("blur command missing"),
        }
    );

    let shadow = tree.create_drop_shadow(DropShadowEffect::new(
        Offset::new(0., 4.),
        5.,
        Color::rgba(0, 0, 0, 128),
    ));
    tree.set_children(shadow, vec![blur]);
    tree.set_root(shadow);
    let first_shadow = tree.flatten();
    let first_shadow_generation = match first_shadow.commands().first() {
        Some(PaintCommand::PushDropShadow { generation, .. }) => *generation,
        _ => panic!("shadow command missing"),
    };
    assert!(tree.update_drop_shadow(
        shadow,
        DropShadowEffect::new(Offset::new(8., -2.), 5., Color::rgba(200, 20, 40, 90),)
    ));
    let second_shadow = tree.flatten();
    assert_eq!(
        first_shadow_generation,
        match second_shadow.commands().first() {
            Some(PaintCommand::PushDropShadow { generation, .. }) => *generation,
            _ => panic!("shadow command missing"),
        }
    );
}

#[test]
fn rounded_rect_contains_uses_normalized_corner_arcs() {
    let rect = RRect::uniform(
        Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        8.,
    );
    assert!(rect.contains(Offset::new(10., 10.)));
    assert!(rect.contains(Offset::new(0., 10.)));
    assert!(rect.contains(Offset::new(3., 3.)));
    assert!(!rect.contains(Offset::new(1., 1.)));
    assert!(rect.contains(Offset::new(8., 0.)));
}

#[test]
fn path_contains_honors_even_odd_holes() {
    let mut b = Path::builder();
    b.move_to(Offset::new(0., 0.))
        .line_to(Offset::new(20., 0.))
        .line_to(Offset::new(20., 20.))
        .line_to(Offset::new(0., 20.))
        .close();
    b.move_to(Offset::new(5., 5.))
        .line_to(Offset::new(15., 5.))
        .line_to(Offset::new(15., 15.))
        .line_to(Offset::new(5., 15.))
        .close();
    let path = b.build();
    assert!(path.contains(Offset::new(2., 2.), FillRule::EvenOdd));
    assert!(!path.contains(Offset::new(10., 10.), FillRule::EvenOdd));
}
#[test]
fn gradient_sampling_preserves_middle_stops_duplicate_edges_and_alpha() {
    let stops = GradientStops::new(vec![
        GradientStop {
            offset: 0.,
            color: Color::rgba(255, 0, 0, 255),
        },
        GradientStop {
            offset: 0.5,
            color: Color::rgba(0, 255, 0, 0),
        },
        GradientStop {
            offset: 1.,
            color: Color::rgba(0, 0, 255, 255),
        },
    ]);
    assert_eq!(sample_gradient_stops(&stops, 0.), [1., 0., 0., 1.]);
    assert_eq!(sample_gradient_stops(&stops, 1.), [0., 0., 1., 1.]);
    let quarter = sample_gradient_stops(&stops, 0.25);
    assert!(
        quarter[0] > 0.4 && quarter[3] > 0.4,
        "premultiplied transparent transition"
    );
    let hard = GradientStops::new(vec![
        GradientStop {
            offset: 0.,
            color: Color::rgba(255, 0, 0, 255),
        },
        GradientStop {
            offset: 0.5,
            color: Color::rgba(255, 0, 0, 255),
        },
        GradientStop {
            offset: 0.5,
            color: Color::rgba(0, 0, 255, 255),
        },
        GradientStop {
            offset: 1.,
            color: Color::rgba(0, 0, 255, 255),
        },
    ]);
    assert_eq!(sample_gradient_stops(&hard, 0.5), [0., 0., 1., 1.]);
    assert_eq!(sample_gradient_stops(&hard, -1.), [1., 0., 0., 1.]);
    assert_eq!(sample_gradient_stops(&hard, 2.), [0., 0., 1., 1.]);
}
#[test]
fn gradient_local_geometry_handles_regular_and_degenerate_cases() {
    assert_eq!(
        linear_gradient_t(Offset::ZERO, Offset::new(10., 0.), Offset::new(2.5, 4.)),
        0.25
    );
    assert_eq!(
        linear_gradient_t(Offset::ZERO, Offset::ZERO, Offset::new(2., 2.)),
        1.
    );
    assert_eq!(
        radial_gradient_t(Offset::ZERO, 10., Offset::new(3., 4.)),
        0.5
    );
    assert_eq!(radial_gradient_t(Offset::ZERO, 0., Offset::ZERO), 1.);
}

#[test]
fn sweep_gradient_wraps_angles_and_canvas_records_oval_clips() {
    assert!((sweep_gradient_t(Offset::ZERO, 0., Offset::new(1., 0.))).abs() < 1e-6);
    assert!((sweep_gradient_t(Offset::ZERO, 0., Offset::new(0., 1.)) - 0.25).abs() < 1e-6);
    let mut canvas = Canvas::default();
    canvas.save_clip_oval(Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)));
    canvas.restore();
    assert!(matches!(
        canvas.finish().commands()[0],
        PaintCommand::PushClipOval { .. }
    ));
}

#[test]
fn color_filter_helpers_cover_identity_alpha_and_common_adjustments() {
    let sample = [0.2, 0.4, 0.8, 0.5];
    assert_eq!(ColorFilter::identity().apply(sample), sample);
    assert!(ColorFilter::grayscale(0.).is_identity());
    let gray = ColorFilter::grayscale(1.).apply(sample);
    assert!((gray[0] - gray[1]).abs() < 1e-6);
    assert!((gray[1] - gray[2]).abs() < 1e-6);
    assert_eq!(ColorFilter::brightness(0.).apply(sample), [0., 0., 0., 0.5]);
    assert!(ColorFilter::contrast(1.).is_identity());
    assert!(ColorFilter::saturate(1.).is_identity());
    let inverted = ColorFilter::invert(1.).apply(sample);
    assert!((inverted[0] - 0.8).abs() < 1e-6);
    assert!((inverted[1] - 0.6).abs() < 1e-6);
    assert!((inverted[2] - 0.2).abs() < 1e-6);
    assert_eq!(ColorFilter::opacity(0.).apply(sample), [0., 0., 0., 0.]);
    let transparent = ColorFilter::invert(1.).apply([1., 0.5, 0.25, 0.]);
    assert_eq!(transparent, [0., 0., 0., 0.]);
    for value in ColorFilter::matrix([f32::NAN; 20]).apply(sample) {
        assert!(value.is_finite());
    }
}

#[test]
fn color_filter_composition_preserves_followed_by_order_and_bias() {
    let first = ColorFilter::matrix([
        0.5, 0., 0., 0., 0.1, 0., 0.5, 0., 0., 0.2, 0., 0., 0.5, 0., 0.3, 0., 0., 0., 1., 0.,
    ]);
    let second = ColorFilter::matrix([
        1., 0., 0., 0., 0.05, 0., 1., 0., 0., 0.06, 0., 0., 1., 0., 0.07, 0., 0., 0., 1., 0.,
    ]);
    let input = [0.4, 0.5, 0.6, 0.8];
    let sequential = second.apply(first.apply(input));
    let composed = first.then(second).apply(input);
    for (left, right) in sequential.into_iter().zip(composed) {
        assert!((left - right).abs() < 1e-6, "{left} != {right}");
    }
    assert_eq!(first.compose(second), first.then(second));
}

#[test]
fn effect_chain_fuses_only_adjacent_color_matrices() {
    let chain = EffectChain::new()
        .color_filter(ColorFilter::grayscale(1.))
        .color_filter(ColorFilter::sepia(0.5))
        .blur(8.)
        .color_filter(ColorFilter::contrast(1.2));
    let optimized = chain.optimized();
    assert_eq!(chain.fusion_count(), 1);
    assert_eq!(optimized.effects().len(), 3);
    assert!(matches!(optimized.effects()[0], Effect::ColorMatrix(_)));
    assert!(matches!(optimized.effects()[1], Effect::GaussianBlur(_)));
    assert!(matches!(optimized.effects()[2], Effect::ColorMatrix(_)));
}

#[test]
fn blend_reference_is_finite_for_transparent_and_semitransparent_pixels() {
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
    for mode in modes {
        for (source, destination) in [
            ([0., 0., 0., 0.], [0., 0., 0., 0.]),
            ([0.2, 0.1, 0.05, 0.5], [0.3, 0.2, 0.1, 0.5]),
            ([0.1, 0.3, 0.2, 1.], [0.4, 0.1, 0.7, 1.]),
        ] {
            let output = blend_premultiplied(mode, source, destination);
            assert!(output.iter().all(|value| value.is_finite()));
            assert!(output[..3].iter().all(|value| *value <= output[3] + 1e-6));
        }
    }
}

#[test]
fn color_filter_and_blend_updates_keep_source_generations_stable() {
    let mut tree = LayerTree::new();
    let leaf = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
    );
    let filter = tree.create_color_filter(ColorFilter::grayscale(1.));
    let blend = tree.create_blend(BlendMode::Multiply);
    tree.set_children(filter, vec![leaf]);
    tree.set_children(blend, vec![filter]);
    tree.set_root(blend);
    let first = tree.flatten();
    let generations = first
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::PushColorFilter {
                layer, generation, ..
            }
            | PaintCommand::PushBlend {
                layer, generation, ..
            } => Some((*layer, *generation)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(generations.len(), 2);
    assert!(tree.update_color_filter(filter, ColorFilter::sepia(1.)));
    assert!(tree.update_blend(blend, BlendMode::Screen));
    let second = tree.flatten();
    let updated = second
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::PushColorFilter {
                layer, generation, ..
            }
            | PaintCommand::PushBlend {
                layer, generation, ..
            } => Some((*layer, *generation)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(generations[1], updated[1]);
    assert_ne!(generations[0], updated[0]);
}

#[test]
fn ordered_effect_generations_invalidate_only_downstream_stages() {
    fn generations(list: &DisplayList) -> Vec<(LayerId, u64)> {
        list.commands()
            .iter()
            .filter_map(|command| match command {
                PaintCommand::PushBlur {
                    layer, generation, ..
                }
                | PaintCommand::PushColorFilter {
                    layer, generation, ..
                } => Some((*layer, *generation)),
                _ => None,
            })
            .collect()
    }

    let make_picture = |tree: &mut LayerTree| {
        tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        )
    };

    // Color -> blur: the blur's input generation includes the color stage.
    let mut color_then_blur = LayerTree::new();
    let source = make_picture(&mut color_then_blur);
    let color = color_then_blur.create_color_filter(ColorFilter::grayscale(0.));
    let blur = color_then_blur.create_blur(GaussianBlur::uniform(8.));
    color_then_blur.set_children(color, vec![source]);
    color_then_blur.set_children(blur, vec![color]);
    color_then_blur.set_root(blur);
    let before = generations(&color_then_blur.flatten());
    assert!(color_then_blur.update_color_filter(color, ColorFilter::grayscale(1.)));
    let after = generations(&color_then_blur.flatten());
    assert_ne!(before[0].1, after[0].1);
    assert_eq!(before[1].1, after[1].1);

    // Blur -> color: changing the downstream matrix does not invalidate
    // the upstream blur source generation.
    let mut blur_then_color = LayerTree::new();
    let source = make_picture(&mut blur_then_color);
    let blur = blur_then_color.create_blur(GaussianBlur::uniform(8.));
    let color = blur_then_color.create_color_filter(ColorFilter::grayscale(0.));
    blur_then_color.set_children(blur, vec![source]);
    blur_then_color.set_children(color, vec![blur]);
    blur_then_color.set_root(color);
    let before = generations(&blur_then_color.flatten());
    assert!(blur_then_color.update_color_filter(color, ColorFilter::grayscale(1.)));
    let after = generations(&blur_then_color.flatten());
    assert_eq!(before[0].1, after[0].1);
    assert_eq!(before[1].1, after[1].1);
}

#[test]
fn kurbo_provides_tight_bezier_bounds_and_winding() {
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(0., 0.))
        .quadratic_to(Offset::new(10., 20.), Offset::new(20., 0.))
        .line_to(Offset::new(0., 0.))
        .close();
    let path = builder.build();
    let bounds = path.bounds().expect("non-empty path");
    // The control point reaches y=20, while the actual quadratic maximum
    // is y=10. This guards against restoring the old control-point box.
    assert!((bounds.size.height - 10.).abs() < 0.0001, "{bounds:?}");
    assert!(path.contains(Offset::new(10., 5.), FillRule::NonZero));
    assert!(!path.contains(Offset::new(10., 12.), FillRule::NonZero));
}

#[test]
fn retained_layers_compose_affines_for_world_bounds() {
    let mut tree = LayerTree::new();
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::ZERO, Size::new(10., 20.)),
    );
    let rotate = tree.create_transform(Transform::rotation(std::f32::consts::FRAC_PI_2));
    tree.set_children(rotate, vec![picture]);
    tree.set_root(rotate);
    let _ = tree.flatten();
    let bounds = tree.flattened_pictures()[0].world_bounds;
    assert!((bounds.size.width - 20.).abs() < 0.0001);
    assert!((bounds.size.height - 10.).abs() < 0.0001);
}
