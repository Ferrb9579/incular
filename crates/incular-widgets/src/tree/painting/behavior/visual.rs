use super::super::*;

impl WidgetTree {
    pub(super) fn paint_visual_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        size: Size,
        cache: &mut DisplayList,
    ) -> Option<Color> {
        let focus_ring = match &kind {
            RenderKind::Button {
                color,
                focused_color,
                enabled,
                ..
            } => (self.button_state_live(id).focused && *enabled && color.alpha == 0)
                .then_some(*focused_color)
                .flatten(),
            _ => None,
        };
        match kind {
            RenderKind::Box { color, .. } => {
                if color.alpha > 0 {
                    cache.push(PaintCommand::Rect {
                        rect: Rect::from_origin_size(Offset::ZERO, size),
                        color,
                    });
                }
            }
            RenderKind::Shape {
                path, fill, stroke, ..
            } => {
                if let Some(brush) = fill {
                    cache.push(PaintCommand::FillPath {
                        path: path.clone(),
                        brush,
                        fill_rule: FillRule::NonZero,
                    });
                }
                if let Some((brush, stroke)) = stroke {
                    cache.push(PaintCommand::StrokePath {
                        path,
                        brush,
                        stroke,
                    });
                }
            }
            RenderKind::CustomPaint { display_list, .. } => {
                cache.extend_from(&display_list);
            }
            RenderKind::Decorated {
                background,
                border,
                radius,
                ..
            } => {
                let rrect = RRect::new(Rect::from_origin_size(Offset::ZERO, size), radius);
                if let Some(brush) = background {
                    cache.push(PaintCommand::RRect { rrect, brush });
                }
                if let Some(border) = border {
                    cache.push(PaintCommand::Border { rrect, border });
                }
            }
            RenderKind::Banner {
                location,
                layout_direction,
                color,
                text_style,
                ..
            } => {
                let geometry = crate::utilities::banner_geometry(size, location, layout_direction);
                cache.push(PaintCommand::PushTransform {
                    transform: CoreTransform::translation(geometry.translation),
                });
                cache.push(PaintCommand::PushTransform {
                    transform: CoreTransform::rotation(geometry.rotation),
                });
                if color.alpha > 0 {
                    cache.push(PaintCommand::Rect {
                        rect: geometry.rect,
                        color,
                    });
                }
                if let Some(layout) = self
                    .render_live(id, "retained render must remain live")
                    .text_layout_cloned()
                {
                    let text_offset = Offset::new(
                        geometry.rect.origin.x,
                        geometry.rect.origin.y
                            + (geometry.rect.size.height - layout.metrics.size.height) * 0.5,
                    );
                    cache.push(PaintCommand::PushTransform {
                        transform: CoreTransform::translation(text_offset),
                    });
                    for line in layout.lines.iter() {
                        for run in line.runs.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: run.clone(),
                                color: text_style.color,
                            });
                        }
                    }
                    cache.push(PaintCommand::PopTransform);
                }
                cache.push(PaintCommand::PopTransform);
                cache.push(PaintCommand::PopTransform);
            }
            RenderKind::Button {
                color,
                hover_color,
                pressed_color,
                focused_color,
                disabled_color,
                enabled,
                ..
            } => {
                let button = self.button_state_live(id);
                // Transparent buttons are commonly used as the retained
                // hit/semantic surface for compound controls (checkboxes,
                // switches, toggles, and radios).  Treat their focused
                // color as a focus ring instead of filling the entire
                // hit surface.  Filling a label row with the accent made
                // keyboard focus look like a stuck hover highlight and
                // obscured the control's actual state.
                let state_color = if button.pressed {
                    pressed_color.or(hover_color).or(focused_color)
                } else if button.hovered {
                    hover_color
                } else if button.focused {
                    focus_ring.map(|_| Color::TRANSPARENT)
                } else {
                    None
                };
                let base = if !enabled {
                    disabled_color.or(Some(color))
                } else {
                    state_color.or(Some(color))
                }
                .unwrap_or(color);
                if base.alpha > 0 {
                    cache.push(PaintCommand::RRect {
                        rrect: RRect::uniform(Rect::from_origin_size(Offset::ZERO, size), 4.),
                        brush: base.into(),
                    });
                }
            }
            _ => unreachable!("visual paint received a non-visual render kind"),
        }
        focus_ring
    }
}
