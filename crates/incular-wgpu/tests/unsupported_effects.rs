//! Explicit backend outcomes for retained stages without execution:
//! shader masks and enabled backdrop filters fail the frame with a named
//! error instead of drawing their children unaffected. Disabled and
//! zero-sigma backdrop filters are supported passthroughs and report no
//! error.
//!
//! These run against the single decision site the lowering arm consults,
//! so predicate and renderer cannot disagree. CPU-deterministic, no
//! device needed.

use incular_core::{Color, Offset, Rect, Size, Transform};
use incular_rendering::{BlendMode, Brush, DisplayList, GaussianBlur, LayerTree, PaintCommand};
use incular_wgpu::{RendererError, unsupported_effect};

fn mask_command() -> PaintCommand {
    let mut tree = LayerTree::new();
    let layer = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.)),
    );
    PaintCommand::PushShaderMask {
        layer,
        shader: Brush::Solid(Color::WHITE),
        blend_mode: BlendMode::Modulate,
        mask_size: Size::new(40., 40.),
        mask_transform: Transform::IDENTITY,
        generation: 1,
        bounds: Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.)),
    }
}

fn backdrop_command(sigma: f32, enabled: bool) -> PaintCommand {
    let mut tree = LayerTree::new();
    let layer = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.)),
    );
    PaintCommand::PushBackdropFilter {
        layer,
        blur: GaussianBlur::uniform(sigma),
        blend_mode: BlendMode::SrcOver,
        enabled,
        generation: 1,
        bounds: Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.)),
    }
}

#[test]
fn shader_mask_reports_unsupported_effect() {
    let error = unsupported_effect(&mask_command()).expect("mask must report");
    assert!(
        matches!(error, RendererError::UnsupportedShaderMask),
        "unexpected outcome: {error:?}"
    );
    assert!(error.to_string().contains("shader masks are not executed"));
}

#[test]
fn enabled_backdrop_filter_reports_unsupported_effect() {
    let error =
        unsupported_effect(&backdrop_command(4., true)).expect("enabled filter must report");
    assert!(
        matches!(error, RendererError::UnsupportedBackdropFilter),
        "unexpected outcome: {error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("backdrop filters are not executed")
    );
}

#[test]
fn disabled_and_zero_sigma_backdrops_pass_through() {
    assert!(
        unsupported_effect(&backdrop_command(4., false)).is_none(),
        "disabled filter is a supported passthrough"
    );
    assert!(
        unsupported_effect(&backdrop_command(0., true)).is_none(),
        "zero sigma is a supported passthrough"
    );
}

#[test]
fn ordinary_commands_have_no_unsupported_outcome() {
    let rect = PaintCommand::Rect {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 4.)),
        color: Color::WHITE,
    };
    assert!(unsupported_effect(&rect).is_none());
    let clip = PaintCommand::PushClip {
        rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 4.)),
    };
    assert!(unsupported_effect(&clip).is_none());
}
