use incular_config::TransparencyMode;
use incular_core::Color;
use incular_wgpu::{SurfaceAlphaError, SurfaceAlphaPlan, SurfaceAlphaRepresentation};
use wgpu::CompositeAlphaMode;

#[test]
fn opaque_windows_select_an_explicit_opaque_mode() {
    let plan = SurfaceAlphaPlan::select(
        TransparencyMode::Opaque,
        &[
            CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::Inherit,
        ],
    )
    .expect("opaque mode");
    assert_eq!(plan.composite_mode(), CompositeAlphaMode::Opaque);
    assert_eq!(plan.representation(), SurfaceAlphaRepresentation::Opaque);
    assert!(!plan.requires_straight_alpha_conversion());
}

#[test]
fn transparent_windows_prefer_zero_copy_premultiplied_presentation() {
    let plan = SurfaceAlphaPlan::select(
        TransparencyMode::Transparent,
        &[
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::PreMultiplied,
        ],
    )
    .expect("transparent mode");
    assert_eq!(plan.composite_mode(), CompositeAlphaMode::PreMultiplied);
    assert_eq!(
        plan.representation(),
        SurfaceAlphaRepresentation::Premultiplied
    );
    assert!(!plan.requires_straight_alpha_conversion());
}

#[test]
fn transparent_windows_support_postmultiplied_surfaces_with_conversion() {
    let plan = SurfaceAlphaPlan::select(
        TransparencyMode::Transparent,
        &[
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PostMultiplied,
        ],
    )
    .expect("postmultiplied transparent mode");
    assert_eq!(plan.composite_mode(), CompositeAlphaMode::PostMultiplied);
    assert_eq!(plan.representation(), SurfaceAlphaRepresentation::Straight);
    assert!(plan.requires_straight_alpha_conversion());
}

#[test]
fn transparent_windows_never_use_opaque_auto_or_inherit_as_a_fallback() {
    assert_eq!(
        SurfaceAlphaPlan::select(
            TransparencyMode::Transparent,
            &[
                CompositeAlphaMode::Auto,
                CompositeAlphaMode::Opaque,
                CompositeAlphaMode::Inherit,
            ],
        ),
        Err(SurfaceAlphaError::TransparentCompositingUnsupported)
    );
}

#[test]
fn opaque_windows_use_inherit_only_when_explicit_opaque_is_unavailable() {
    let plan = SurfaceAlphaPlan::select(
        TransparencyMode::Opaque,
        &[
            CompositeAlphaMode::Inherit,
            CompositeAlphaMode::PostMultiplied,
        ],
    )
    .expect("inherited opaque baseline");
    assert_eq!(plan.composite_mode(), CompositeAlphaMode::Inherit);
    assert_eq!(plan.representation(), SurfaceAlphaRepresentation::Opaque);
}

#[test]
fn invalid_opaque_capabilities_do_not_fabricate_an_alpha_mode() {
    assert_eq!(
        SurfaceAlphaPlan::select(
            TransparencyMode::Opaque,
            &[CompositeAlphaMode::PostMultiplied],
        ),
        Err(SurfaceAlphaError::NoOpaqueCompositingMode)
    );
}

#[test]
fn premultiplied_linear_readback_becomes_straight_rgba8() {
    let rgba = SurfaceAlphaRepresentation::Premultiplied
        .to_straight_rgba8(wgpu::TextureFormat::Rgba8Unorm, [64, 32, 16, 128]);
    assert_eq!(rgba, [128, 64, 32, 128]);
    assert_eq!(
        SurfaceAlphaRepresentation::Premultiplied
            .to_straight_rgba8(wgpu::TextureFormat::Rgba8Unorm, [91, 42, 17, 0]),
        [0, 0, 0, 0]
    );
}

#[test]
fn premultiplied_srgb_readback_unpremultiplies_in_linear_light() {
    let original = Color::rgba(201, 103, 47, 128);
    let [red, green, blue, alpha] = original.to_linear_rgba();
    let stored = Color::from_linear_rgba([red * alpha, green * alpha, blue * alpha, alpha]);
    let converted = SurfaceAlphaRepresentation::Premultiplied.to_straight_rgba8(
        wgpu::TextureFormat::Rgba8UnormSrgb,
        [stored.red, stored.green, stored.blue, stored.alpha],
    );
    for (actual, expected) in
        converted
            .into_iter()
            .zip([original.red, original.green, original.blue, original.alpha])
    {
        assert!(actual.abs_diff(expected) <= 1, "{actual} != {expected}");
    }
}

#[test]
fn straight_surface_readback_is_not_unpremultiplied_again() {
    let rgba = [190, 70, 20, 96];
    assert_eq!(
        SurfaceAlphaRepresentation::Straight
            .to_straight_rgba8(wgpu::TextureFormat::Rgba8UnormSrgb, rgba),
        rgba
    );
}
