use super::super::*;

impl WidgetTree {
    pub(super) fn paint_content_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        size: Size,
        cache: &mut DisplayList,
    ) {
        match kind {
            RenderKind::Text {
                style, overflow, ..
            } => {
                if let Some(layout) = self.renders.get(id.0).expect("live").text_layout_cloned() {
                    if overflow == TextOverflow::Clip {
                        cache.push(PaintCommand::PushClip {
                            rect: Rect::from_origin_size(Offset::ZERO, size),
                        });
                    }
                    for line in layout.lines.iter() {
                        for run in line.runs.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: run.clone(),
                                color: style.color,
                            });
                        }
                    }
                    if overflow == TextOverflow::Clip {
                        cache.push(PaintCommand::PopClip);
                    }
                }
            }
            RenderKind::SelectableText { style, .. } => {
                let selection = self
                    .element_for_render(id)
                    .and_then(|element| self.static_selection_range(element));
                if let (Some(layout), Some(selection)) = (
                    self.renders.get(id.0).expect("live").text_layout_cloned(),
                    selection,
                ) {
                    for rect in selection_rects(&layout, selection, 0., 0., 0.) {
                        cache.push(PaintCommand::Rect {
                            rect,
                            color: Color::rgba(72, 120, 220, 150),
                        });
                    }
                    for line in layout.lines.iter() {
                        for run in line.runs.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: run.clone(),
                                color: style.color,
                            });
                        }
                    }
                } else if let Some(layout) =
                    self.renders.get(id.0).expect("live").text_layout_cloned()
                {
                    for line in layout.lines.iter() {
                        for run in line.runs.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: run.clone(),
                                color: style.color,
                            });
                        }
                    }
                }
            }
            RenderKind::SelectionArea
            | RenderKind::SelectionContainer
            | RenderKind::SelectionListener
            | RenderKind::IndexedSemantics
            | RenderKind::SemanticsDebugger { .. } => {}
            RenderKind::Image {
                image,
                fit,
                repeat,
                alignment,
                sampling,
                ..
            } => {
                let intrinsic = image.decoded();
                let source = Rect::from_origin_size(
                    Offset::ZERO,
                    Size::new(intrinsic.width() as f32, intrinsic.height() as f32),
                );
                let (source, destination) = image_fit_rects(
                    source,
                    Rect::from_origin_size(Offset::ZERO, size),
                    fit,
                    alignment,
                );
                for destination in image_repeat_destinations(
                    destination,
                    Rect::from_origin_size(Offset::ZERO, size),
                    repeat,
                ) {
                    cache.push(PaintCommand::Image {
                        image: image.clone(),
                        source,
                        destination,
                        sampling,
                    });
                }
            }
            RenderKind::TextField {
                controller,
                style,
                placeholder,
                multiline,
                obscure_text,
                cursor_width,
                cursor_height,
                cursor_radius,
                show_cursor,
                cursor_color,
                selection_color,
                ..
            } => {
                let (layout, focused, scroll_x, scroll_y) = {
                    let node = self.renders.get(id.0).expect("live");
                    let state = node
                        .text_field_state()
                        .expect("text-field render must own text-field state");
                    (
                        node.text_layout_cloned(),
                        state.focused,
                        state.scroll_x,
                        state.scroll_y,
                    )
                };
                let value = controller.value();
                let display = text_field_display(&controller, &placeholder, obscure_text);
                let mut active_scroll_x = scroll_x;
                let mut active_scroll_y = scroll_y;
                if let Some(layout) = layout {
                    let (caret, caret_y, caret_height) = caret_geometry(
                        &layout,
                        value.selection.extent,
                        controller.caret_affinity(),
                    );
                    let available = (size.width - 16.).max(0.);
                    if !multiline && caret - active_scroll_x > available {
                        active_scroll_x = caret - available;
                    } else if !multiline && caret < active_scroll_x {
                        active_scroll_x = caret;
                    }
                    active_scroll_x = active_scroll_x.max(0.);
                    let top = if multiline {
                        8.
                    } else {
                        ((size.height - layout.metrics.line_height) / 2.).max(0.)
                    };
                    if multiline {
                        let viewport = (size.height - 16.).max(0.);
                        if caret_y - active_scroll_y < 0. {
                            active_scroll_y = caret_y;
                        } else if caret_y + caret_height - active_scroll_y > viewport {
                            active_scroll_y = caret_y + caret_height - viewport;
                        }
                        active_scroll_y = active_scroll_y
                            .clamp(0., (layout.metrics.size.height - viewport).max(0.));
                    }
                    let selection = value.selection.range();
                    if focused && !selection.is_empty() {
                        for rect in selection_rects(
                            &layout,
                            selection,
                            active_scroll_x,
                            active_scroll_y,
                            top,
                        ) {
                            cache.push(PaintCommand::Rect {
                                rect,
                                color: selection_color,
                            });
                        }
                    }
                    cache.push(PaintCommand::PushClip {
                        rect: Rect::from_origin_size(
                            Offset::new(8., 2.),
                            Size::new((size.width - 16.).max(0.), (size.height - 4.).max(0.)),
                        ),
                    });
                    cache.push(PaintCommand::PushTransform {
                        transform: CoreTransform::translation(Offset::new(
                            8. - active_scroll_x,
                            top - active_scroll_y,
                        )),
                    });
                    let color = if display == placeholder && value.text.is_empty() {
                        Color::rgba(150, 154, 170, 255)
                    } else {
                        style.color
                    };
                    for line in layout.lines.iter() {
                        for run in line.runs.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: run.clone(),
                                color,
                            });
                        }
                    }
                    cache.push(PaintCommand::PopTransform);
                    cache.push(PaintCommand::PopClip);
                    if focused && show_cursor && controller.caret_visible(Instant::now()) {
                        let height = cursor_height.unwrap_or(caret_height).max(0.0);
                        let y = top + caret_y - active_scroll_y
                            + (caret_height - height).max(0.0) * 0.5;
                        let rect = Rect::from_origin_size(
                            Offset::new(caret - active_scroll_x + 8., y),
                            Size::new(cursor_width.max(0.0), height),
                        );
                        if cursor_radius > 0.0 {
                            cache.push(PaintCommand::RRect {
                                rrect: RRect::uniform(rect, cursor_radius),
                                brush: cursor_color.into(),
                            });
                        } else if rect.size.width > 0.0 && rect.size.height > 0.0 {
                            cache.push(PaintCommand::Rect {
                                rect,
                                color: cursor_color,
                            });
                        }
                    }
                }
                let node = self.renders.get_mut(id.0).expect("live");
                let state = node
                    .text_field_state_mut()
                    .expect("text-field render must own text-field state");
                state.scroll_x = active_scroll_x;
                state.scroll_y = active_scroll_y;
            }
            _ => unreachable!("content paint received a non-content render kind"),
        }
    }
}
