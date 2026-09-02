use super::prelude::*;
use super::*;

pub(crate) fn find_opacity_end(
    commands: &[PaintCommand],
    start: usize,
) -> Result<usize, RendererError> {
    let mut depth = 0_usize;
    for (index, command) in commands.iter().enumerate().skip(start) {
        match command {
            PaintCommand::PushOpacity { .. } => depth += 1,
            PaintCommand::PopOpacity => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(RendererError::UnbalancedClipStack)
}

pub(crate) fn find_effect_end(
    commands: &[PaintCommand],
    start: usize,
) -> Result<usize, RendererError> {
    let mut depth = 0_usize;
    for (index, command) in commands.iter().enumerate().skip(start) {
        match command {
            PaintCommand::PushBlur { .. }
            | PaintCommand::PushDropShadow { .. }
            | PaintCommand::PushColorFilter { .. }
            | PaintCommand::PushBlend { .. }
            | PaintCommand::PushShaderMask { .. }
            | PaintCommand::PushBackdropFilter { .. } => depth += 1,
            PaintCommand::PopEffect => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(RendererError::UnbalancedClipStack)
}
pub(crate) fn commands_have_effects(commands: &[PaintCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            PaintCommand::PushOpacity { .. }
                | PaintCommand::PushBlur { .. }
                | PaintCommand::PushDropShadow { .. }
                | PaintCommand::PushColorFilter { .. }
                | PaintCommand::PushBlend { .. }
                | PaintCommand::PushShaderMask { .. }
                | PaintCommand::PushBackdropFilter { .. }
        )
    })
}

pub(crate) fn commands_have_destination_blend(commands: &[PaintCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            PaintCommand::PushBlend { mode, .. } if mode.requires_destination_read()
        )
    })
}

pub(crate) fn add_composition_bounds(bounds: &mut Option<Rect>, candidate: Rect) {
    if !candidate.origin.x.is_finite()
        || !candidate.origin.y.is_finite()
        || !candidate.size.width.is_finite()
        || !candidate.size.height.is_finite()
        || candidate.size.width <= 0.
        || candidate.size.height <= 0.
    {
        return;
    }
    *bounds = Some(match bounds.take() {
        Some(current) => {
            let left = current.origin.x.min(candidate.origin.x);
            let top = current.origin.y.min(candidate.origin.y);
            let right = (current.origin.x + current.size.width)
                .max(candidate.origin.x + candidate.size.width);
            let bottom = (current.origin.y + current.size.height)
                .max(candidate.origin.y + candidate.size.height);
            Rect::from_origin_size(
                Offset::new(left, top),
                Size::new(right - left, bottom - top),
            )
        }
        None => candidate,
    });
}

/// Computes a conservative logical scene scope for destination promotion. It
/// deliberately includes every visible command (and expanded blur/shadow
/// bounds), so presenting a tight target cannot omit content outside a small
/// blend group. The ordinary direct path still uses the complete surface.
pub(crate) fn display_list_composition_bounds(commands: &[PaintCommand]) -> Option<Rect> {
    let mut transforms = vec![Offset::ZERO];
    let mut bounds = None;
    for command in commands {
        let translation = *transforms.last().expect("root transform");
        match command {
            PaintCommand::Rect { rect, .. }
            | PaintCommand::Image {
                destination: rect, ..
            } => add_composition_bounds(&mut bounds, translated_rect(*rect, translation)),
            PaintCommand::RRect { rrect, .. } | PaintCommand::Border { rrect, .. } => {
                add_composition_bounds(&mut bounds, translated_rect(rrect.rect, translation));
            }
            PaintCommand::FillPath { path, .. } | PaintCommand::StrokePath { path, .. } => {
                if let Some(path_bounds) = path.bounds() {
                    let margin = match command {
                        PaintCommand::StrokePath { stroke, .. } => {
                            (stroke.width.max(0.) * 0.5 * stroke.miter_limit.max(1.)).max(0.)
                        }
                        _ => 0.,
                    };
                    add_composition_bounds(
                        &mut bounds,
                        Rect::from_origin_size(
                            Offset::new(
                                path_bounds.origin.x - margin + translation.x,
                                path_bounds.origin.y - margin + translation.y,
                            ),
                            Size::new(
                                path_bounds.size.width + margin * 2.,
                                path_bounds.size.height + margin * 2.,
                            ),
                        ),
                    );
                }
            }
            PaintCommand::GlyphRun { run, .. } => {
                if let Some((left, right)) = run
                    .glyphs
                    .iter()
                    .map(|glyph| (glyph.offset.x, glyph.offset.x + glyph.advance.max(0.)))
                    .reduce(|(left, right), (next_left, next_right)| {
                        (left.min(next_left), right.max(next_right))
                    })
                {
                    let font_size = run.font_size.abs().max(1.);
                    let x_margin = font_size * 0.25;
                    add_composition_bounds(
                        &mut bounds,
                        Rect::from_origin_size(
                            Offset::new(
                                run.origin.x + left - x_margin + translation.x,
                                run.origin.y - font_size * 1.35 + translation.y,
                            ),
                            Size::new((right - left + x_margin * 2.).max(1.), font_size * 1.7),
                        ),
                    );
                }
            }
            // Effect bounds emitted by LayerTree are already in the current
            // world coordinate space. Expand only the effects that can paint
            // outside their source rectangle.
            PaintCommand::PushOpacity { bounds: rect, .. }
            | PaintCommand::PushColorFilter { bounds: rect, .. }
            | PaintCommand::PushBlend { bounds: rect, .. } => {
                add_composition_bounds(&mut bounds, *rect);
            }
            PaintCommand::PushBlur {
                bounds: rect, blur, ..
            } => {
                add_composition_bounds(&mut bounds, blur_bounds(*rect, blur.sigma_x, blur.sigma_y))
            }
            PaintCommand::PushDropShadow {
                bounds: rect,
                shadow,
                ..
            } => add_composition_bounds(
                &mut bounds,
                drop_shadow_bounds(*rect, shadow.offset, shadow.sigma_x, shadow.sigma_y),
            ),
            PaintCommand::PushShaderMask { bounds: rect, .. } => {
                add_composition_bounds(&mut bounds, *rect);
            }
            PaintCommand::PushBackdropFilter {
                bounds: rect,
                blur,
                enabled,
                ..
            } => {
                let candidate = if *enabled {
                    blur_bounds(*rect, blur.sigma_x, blur.sigma_y)
                } else {
                    *rect
                };
                add_composition_bounds(&mut bounds, candidate);
            }
            PaintCommand::PushTransform { transform } => {
                transforms.push(translation + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                if transforms.len() > 1 {
                    transforms.pop();
                }
            }
            PaintCommand::PushClip { .. }
            | PaintCommand::PushSurfacePartition { .. }
            | PaintCommand::PopSurfacePartition
            | PaintCommand::PushClipRRect { .. }
            | PaintCommand::PushClipOval { .. }
            | PaintCommand::PushClipPath { .. }
            | PaintCommand::PopClip
            | PaintCommand::PopOpacity
            | PaintCommand::PopEffect => {}
        }
    }
    bounds
}

pub(crate) fn destination_composition_scope(
    list: &DisplayList,
    scale: f32,
    surface_width: u32,
    surface_height: u32,
) -> (Offset, u32, u32) {
    let full = Rect::from_origin_size(
        Offset::ZERO,
        Size::new(surface_width as f32 / scale, surface_height as f32 / scale),
    );
    let scope = display_list_composition_bounds(list.commands())
        .and_then(|bounds| intersect_rect(bounds, full))
        .unwrap_or(full);
    let left = (scope.origin.x * scale).floor().max(0.) as u32;
    let top = (scope.origin.y * scale).floor().max(0.) as u32;
    let right = ((scope.origin.x + scope.size.width) * scale)
        .ceil()
        .min(surface_width as f32) as u32;
    let bottom = ((scope.origin.y + scope.size.height) * scale)
        .ceil()
        .min(surface_height as f32) as u32;
    if right <= left || bottom <= top {
        return (Offset::ZERO, surface_width.max(1), surface_height.max(1));
    }
    (
        Offset::new(left as f32 / scale, top as f32 / scale),
        right - left,
        bottom - top,
    )
}

pub(crate) fn translated_rect(rect: Rect, offset: Offset) -> Rect {
    Rect::from_origin_size(rect.origin + offset, rect.size)
}
pub(crate) fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.origin.x.max(b.origin.x);
    let top = a.origin.y.max(b.origin.y);
    let right = (a.origin.x + a.size.width).min(b.origin.x + b.size.width);
    let bottom = (a.origin.y + a.size.height).min(b.origin.y + b.size.height);
    (right > left && bottom > top).then(|| {
        Rect::from_origin_size(
            Offset::new(left, top),
            Size::new(right - left, bottom - top),
        )
    })
}
pub(crate) fn set_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    clip: ClipState,
    width: u32,
    height: u32,
    scale: f32,
) -> bool {
    let clip = match clip {
        ClipState::Rect(clip) => Some(clip),
        ClipState::Stencil { rect, .. } => rect,
        ClipState::Empty => return false,
        ClipState::Unbounded => {
            pass.set_scissor_rect(0, 0, width, height);
            return true;
        }
    };
    let Some(clip) = clip else {
        pass.set_scissor_rect(0, 0, width, height);
        return true;
    };
    let x0 = (clip.rect.origin.x * scale).floor().max(0.) as u32;
    let y0 = (clip.rect.origin.y * scale).floor().max(0.) as u32;
    let x1 = ((clip.rect.origin.x + clip.rect.size.width) * scale)
        .ceil()
        .max(0.) as u32;
    let y1 = ((clip.rect.origin.y + clip.rect.size.height) * scale)
        .ceil()
        .max(0.) as u32;
    let x0 = x0.min(width);
    let y0 = y0.min(height);
    let scissor_width = x1.min(width).saturating_sub(x0);
    let scissor_height = y1.min(height).saturating_sub(y0);
    if scissor_width == 0 || scissor_height == 0 {
        return false;
    }
    pass.set_scissor_rect(x0, y0, scissor_width, scissor_height);
    true
}
pub(crate) fn normalized_scale(scale: f64) -> f32 {
    if scale.is_finite() && scale > 0. {
        scale as f32
    } else {
        1.
    }
}
pub(crate) fn physical_debug_size(bounds: Rect, scale: f32) -> (u32, u32) {
    (
        (bounds.size.width.max(0.) * scale).ceil() as u32,
        (bounds.size.height.max(0.) * scale).ceil() as u32,
    )
}
pub(crate) fn us_since(started: std::time::Instant) -> u32 {
    started.elapsed().as_micros().try_into().unwrap_or(u32::MAX)
}
pub(crate) fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}
