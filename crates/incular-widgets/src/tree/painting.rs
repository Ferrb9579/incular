//! Display-list painting, compositing, hit testing, and scrollbar geometry.

use super::*;

use super::rendering::{image_fit_rects, image_repeat_destinations};
use super::text::{caret_geometry, selection_rects, text_field_display};

impl WidgetTree {
    #[must_use]
    pub fn paint(&mut self) -> DisplayList {
        let _phase_guard = self.guard_phase_root(FramePhase::Paint);
        let mut ignored = DisplayList::new();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.paint_render(root, &mut ignored);
        }
        self.compositor.flatten()
    }
    #[must_use]
    pub fn hit_test(&self, point: Offset) -> Option<RenderObjectId> {
        self.root
            .and_then(|id| self.render_id(id))
            .and_then(|id| self.hit_test_render(id, point, Offset::ZERO))
    }
    pub(super) fn paint_render(&mut self, id: RenderObjectId, output: &mut DisplayList) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.paint_render_inner(id, output);
        });
    }

    pub(super) fn paint_render_inner(&mut self, id: RenderObjectId, output: &mut DisplayList) {
        let constraints = self.renders.get(id.0).and_then(|render| render.constraints);
        let _node_guard = self.guard_render(FramePhase::Paint, id, constraints);
        if matches!(
            self.renders.get(id.0).expect("live").kind,
            RenderKind::Visibility { visible: false }
        ) {
            return;
        }
        let (mut offset, cache, children, kind) = {
            let node = self.renders.get(id.0).expect("live");
            (
                node.offset,
                node.cache.clone(),
                node.children.clone(),
                node.kind.clone(),
            )
        };
        if let RenderKind::PersistentHeader {
            ref controller,
            axis,
            reverse,
            pinned,
        } = kind
        {
            offset =
                offset + self.persistent_header_translation(id, controller, axis, reverse, pinned);
        }
        let dirty = self
            .renders
            .get(id.0)
            .expect("live")
            .dirty
            .contains(DirtyFlags::PAINT);
        #[cfg(feature = "devtools")]
        let trace = dirty
            .then(|| self.element_for_render(id))
            .flatten()
            .and_then(|element| self.devtools_trace_begin_element(element, TracePhase::Paint));
        if dirty {
            let kind = self.renders.get(id.0).expect("live").kind.clone();
            let size = self.renders.get(id.0).expect("live").size;
            let focus_picture = self.renders.get(id.0).expect("live").focus_picture;
            let focus_ring = match &kind {
                RenderKind::Button {
                    color,
                    focused_color,
                    enabled,
                    ..
                } => {
                    let render = self.renders.get(id.0).expect("live");
                    (render.button_focused && *enabled && color.alpha == 0)
                        .then_some(*focused_color)
                        .flatten()
                }
                _ => None,
            };
            let mut cache = DisplayList::new();
            let mut focus_cache = DisplayList::new();
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
                RenderKind::Button {
                    color,
                    hover_color,
                    pressed_color,
                    focused_color,
                    disabled_color,
                    enabled,
                    ..
                } => {
                    let render = self.renders.get(id.0).expect("live");
                    // Transparent buttons are commonly used as the retained
                    // hit/semantic surface for compound controls (checkboxes,
                    // switches, toggles, and radios).  Treat their focused
                    // color as a focus ring instead of filling the entire
                    // hit surface.  Filling a label row with the accent made
                    // keyboard focus look like a stuck hover highlight and
                    // obscured the control's actual state.
                    let state_color = if render.button_pressed {
                        pressed_color.or(hover_color).or(focused_color)
                    } else if render.button_hovered {
                        hover_color
                    } else if render.button_focused {
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
                RenderKind::Text {
                    style, overflow, ..
                } => {
                    if let Some(layout) = self.renders.get(id.0).expect("live").text_layout.clone()
                    {
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
                        self.renders.get(id.0).expect("live").text_layout.clone(),
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
                        self.renders.get(id.0).expect("live").text_layout.clone()
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
                RenderKind::SelectionArea => {}
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
                        (
                            node.text_layout.clone(),
                            node.focused,
                            node.text_scroll_x,
                            node.text_scroll_y,
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
                    node.text_scroll_x = active_scroll_x;
                    node.text_scroll_y = active_scroll_y;
                }
                RenderKind::Scroll { controller, .. } => {
                    self.paint_scrollbar(id, size, &controller, &mut cache);
                }
                RenderKind::SliverViewport { config } => {
                    self.paint_scrollbar(id, size, &config.controller, &mut cache);
                }
                _ => {}
            }
            if let Some(focus_ring) = focus_ring.filter(|color| color.alpha > 0) {
                let inset = 1.0;
                let ring_size = Size::new(
                    (size.width - 2. * inset).max(0.),
                    (size.height - 2. * inset).max(0.),
                );
                focus_cache.push(PaintCommand::Border {
                    rrect: RRect::uniform(
                        Rect::from_origin_size(Offset::new(inset, inset), ring_size),
                        4.,
                    ),
                    border: Border::new(2.0, focus_ring),
                });
            }
            let node = self.renders.get_mut(id.0).expect("live");
            node.cache = cache;
            node.dirty.remove(DirtyFlags::PAINT);
            if let Some(picture) = node.picture {
                self.compositor.update_picture(
                    picture,
                    node.cache.clone(),
                    Rect::from_origin_size(Offset::ZERO, node.size),
                );
            }
            if let Some(picture) = focus_picture {
                self.compositor.update_picture(
                    picture,
                    focus_cache,
                    Rect::from_origin_size(Offset::ZERO, node.size),
                );
            }
            self.diagnostics.paints += 1;
            #[cfg(feature = "devtools")]
            if let Some(element) = self.element_for_render(id) {
                if let Some(element) = self.elements.get_mut(element.0) {
                    element.dev.paints += 1;
                }
            }
        }
        output.push(PaintCommand::PushTransform {
            transform: CoreTransform::translation(offset),
        });
        if dirty {
            output.extend_from(&self.renders.get(id.0).expect("live").cache);
        } else {
            self.diagnostics.display_lists_reused += 1;
            output.extend_from(&cache);
        }
        let pushed_clip = match &kind {
            RenderKind::ClipRect { .. }
            | RenderKind::ClipRRect { .. }
            | RenderKind::ClipOval { .. }
            | RenderKind::ClipPath { .. } => {
                let node_size = self.renders.get(id.0).expect("live").size;
                output.push(PaintCommand::PushClip {
                    rect: Rect::from_origin_size(Offset::ZERO, node_size),
                });
                true
            }
            _ => false,
        };
        for child in children {
            self.paint_render(child, output);
        }
        if pushed_clip {
            output.push(PaintCommand::PopClip);
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        output.push(PaintCommand::PopTransform);
    }
    pub(super) fn paint_scrollbar(
        &self,
        id: RenderObjectId,
        size: Size,
        controller: &ScrollController,
        cache: &mut DisplayList,
    ) {
        let style = controller.scrollbar_style();
        let geometry = scrollbar_geometry(size, controller, style);
        if !geometry.visible {
            return;
        }
        let node = self.renders.get(id.0).expect("live");
        if !controller.scrollbar_thumb_visibility()
            && !node.scrollbar_hovered
            && !node.scrollbar_dragging
        {
            return;
        }
        let thumb = style.thumb_color;
        if style.track_color.alpha > 0 {
            cache.push(PaintCommand::RRect {
                rrect: RRect::uniform(geometry.track, style.width * 0.5),
                brush: style.track_color.into(),
            });
        }
        if thumb.alpha > 0 {
            cache.push(PaintCommand::RRect {
                rrect: RRect::uniform(geometry.thumb, style.width * 0.5),
                brush: thumb.into(),
            });
        }
    }
    pub(super) fn hit_test_render(
        &self,
        id: RenderObjectId,
        point: Offset,
        origin: Offset,
    ) -> Option<RenderObjectId> {
        let node = self.renders.get(id.0)?;
        match self
            .element_for_render(id)
            .and_then(|element| self.elements.get(element.0))
            .map(|element| &element.widget.kind)
        {
            Some(WidgetKind::IgnorePointer { ignoring: true, .. }) => return None,
            Some(WidgetKind::AbsorbPointer {
                absorbing: true, ..
            }) => return Some(id),
            _ => {}
        }
        if matches!(
            node.kind,
            RenderKind::Transform { .. }
                | RenderKind::Scale { .. }
                | RenderKind::Rotation { .. }
                | RenderKind::FittedBox { .. }
        ) {
            let current = origin + node.offset;
            let local = self
                .child_content_transform(id)
                .inverse_transform_point(point - current)?;
            let child_size = node
                .children
                .first()
                .and_then(|child| self.renders.get(child.0))
                .map_or(node.size, |child| child.size);
            if !Rect::from_origin_size(Offset::ZERO, child_size).contains(local) {
                return None;
            }
            for child in node.children.iter().rev() {
                if let Some(hit) = self.hit_test_render(*child, local, Offset::ZERO) {
                    return Some(hit);
                }
            }
            return None;
        }
        let current = match &node.kind {
            RenderKind::Translate { controller } => origin + node.offset + controller.offset(),
            RenderKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
            } => {
                origin
                    + node.offset
                    + self.persistent_header_translation(id, controller, *axis, *reverse, *pinned)
            }
            _ => origin + node.offset,
        };
        if !Rect::from_origin_size(current, node.size).contains(point) {
            return None;
        }
        if matches!(node.kind, RenderKind::Visibility { visible: false }) {
            return None;
        }
        let child_origin = match &node.kind {
            RenderKind::Scroll {
                controller,
                axis,
                reverse,
                ..
            } => current + scroll_translation(controller, *axis, *reverse),
            RenderKind::SliverViewport { config } => {
                current + scroll_translation(&config.controller, config.axis, config.reverse)
            }
            RenderKind::Translate { .. } => current,
            _ => current,
        };
        let hit_children: Vec<_> = if matches!(node.kind, RenderKind::SliverViewport { .. }) {
            let pinned = self
                .element_for_render(id)
                .and_then(|element| self.elements.get(element.0))
                .map(|element| element.sliver_pinned_ids.clone())
                .unwrap_or_default();
            let mut ordered = node.children.clone();
            ordered.sort_by_key(|child| {
                let child_element = self.element_for_render(*child);
                child_element
                    .and_then(|element| self.elements.get(element.0))
                    .and_then(|element| element.parent)
                    .and_then(|parent| self.elements.get(parent.0))
                    .and_then(|parent| {
                        parent
                            .children
                            .iter()
                            .position(|candidate| Some(*candidate) == child_element)
                            .and_then(|slot| parent.sliver_child_ids.get(slot))
                    })
                    .map_or(0, |child_id| usize::from(pinned.contains(child_id)))
            });
            ordered
        } else {
            match &node.kind {
                RenderKind::IndexedStack { index, .. } => {
                    node.children.get(*index).copied().into_iter().collect()
                }
                _ => node.children.clone(),
            }
        };
        for child in hit_children.iter().rev() {
            if let Some(hit) = self.hit_test_render(*child, point, child_origin) {
                return Some(hit);
            }
        }
        match self
            .element_for_render(id)
            .and_then(|element| self.elements.get(element.0))
            .map(|element| &element.widget.kind)
        {
            Some(WidgetKind::Gesture { behavior, .. })
                if *behavior == crate::gestures::HitTestBehavior::DeferToChild =>
            {
                None
            }
            _ => Some(id),
        }
    }
    pub(super) fn scrollbar_controller_and_geometry(
        &self,
        render: RenderObjectId,
    ) -> Option<(ScrollController, ScrollbarGeometry)> {
        let node = self.renders.get(render.0)?;
        let controller = match &node.kind {
            RenderKind::Scroll { controller, .. } => controller.clone(),
            RenderKind::SliverViewport { config } => config.controller.clone(),
            _ => return None,
        };
        let mut geometry = scrollbar_geometry(node.size, &controller, controller.scrollbar_style());
        let origin = self.render_viewport_origin(render);
        geometry.track.origin = geometry.track.origin + origin;
        geometry.thumb.origin = geometry.thumb.origin + origin;
        Some((controller, geometry))
    }
    pub(super) fn scrollbar_local_geometry(
        &self,
        render: RenderObjectId,
    ) -> Option<(ScrollController, ScrollbarGeometry)> {
        let node = self.renders.get(render.0)?;
        let controller = match &node.kind {
            RenderKind::Scroll { controller, .. } => controller.clone(),
            RenderKind::SliverViewport { config } => config.controller.clone(),
            _ => return None,
        };
        Some((
            controller.clone(),
            scrollbar_geometry(node.size, &controller, controller.scrollbar_style()),
        ))
    }
    pub(super) fn scrollbar_local_point(
        &self,
        render: RenderObjectId,
        point: Offset,
    ) -> Option<Offset> {
        self.render_world_transform(render)
            .inverse_transform_point(point)
    }
    pub(super) fn scrollbar_at(&self, point: Offset) -> Option<RenderObjectId> {
        self.renders.iter().fold(None, |found, (raw, _)| {
            let render = RenderObjectId(raw);
            found.or_else(|| {
                self.scrollbar_local_geometry(render)
                    .and_then(|(_, geometry)| {
                        self.scrollbar_local_point(render, point).and_then(|point| {
                            (geometry.visible && geometry.track.contains(point)).then_some(render)
                        })
                    })
            })
        })
    }
}
