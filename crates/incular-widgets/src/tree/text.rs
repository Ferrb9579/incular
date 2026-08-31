//! Text-field editing, selectable text, caret geometry, and static selection.

use super::*;
use crate::selection::{
    SelectedContentRange, SelectionGeometry, SelectionHandleType, SelectionPoint, SelectionStatus,
};

impl WidgetTree {
    #[must_use]
    pub fn text_field_at(&self, point: Offset) -> Option<ElementId> {
        let hit = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))?;
        self.text_field_ancestor(hit)
    }
    pub fn set_focused(
        &mut self,
        id: ElementId,
        focused: bool,
        now: Instant,
    ) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live");
        if let Some(button) = node.button_state_mut() {
            button.focused = focused;
            button.visual = if button.pressed {
                ButtonState::Pressed
            } else if button.focused {
                ButtonState::Focused
            } else if button.hovered {
                ButtonState::Hovered
            } else {
                ButtonState::Normal
            };
            node.dirty.insert(DirtyFlags::PAINT);
        }
        if node
            .text_field_state()
            .is_some_and(|state| state.focused != focused)
        {
            node.text_field_state_mut()
                .expect("text field state")
                .focused = focused;
            node.dirty.insert(DirtyFlags::PAINT);
            if focused {
                if let RenderKind::TextField { controller, .. } = &node.object.kind {
                    controller.reset_caret(now);
                }
            }
        }
        if node
            .selectable_text_state()
            .is_some_and(|state| state.focused != focused)
        {
            node.selectable_text_state_mut()
                .expect("selectable text state")
                .focused = focused;
            node.dirty.insert(DirtyFlags::PAINT);
        }
        Ok(())
    }
    #[must_use]
    pub fn is_text_field(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| matches!(element.widget.kind, WidgetKind::TextField { .. }))
    }

    /// Returns whether the field is currently allowed to mutate its
    /// controller through keyboard/IME input.  Read-only fields intentionally
    /// remain focusable so selection and copy still work, while disabled
    /// fields are not editable or focusable.
    #[must_use]
    pub fn text_field_is_editable(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    enabled: true,
                    read_only: false,
                    ..
                }
            )
        })
    }

    #[must_use]
    pub fn text_field_is_read_only(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    enabled: true,
                    read_only: true,
                    ..
                }
            )
        })
    }
    #[must_use]
    pub fn is_multiline_text_field(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    multiline: true,
                    ..
                }
            )
        })
    }
    pub fn text_field_move_vertical(&mut self, id: ElementId, down: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let (
            RenderKind::TextField {
                controller,
                multiline,
                ..
            },
            Some(layout),
        ) = (&node.object.kind, node.text_layout_cloned())
        else {
            return false;
        };
        if !multiline {
            return false;
        }
        let current = controller.value().selection.extent;
        let line = line_for_byte(&layout, current);
        let target_line = if down {
            line.saturating_add(1)
        } else {
            line.saturating_sub(1)
        };
        if layout.lines.get(target_line).is_none() {
            return false;
        }
        let x = controller.preferred_caret_x().unwrap_or_else(|| {
            line_caret_x_for_affinity(&layout, line, current, controller.caret_affinity())
        });
        let (target, affinity) = caret_for_line_position(&layout, target_line, x);
        controller.move_cursor_with_affinity(target, affinity, extend);
        controller.set_preferred_caret_x(x);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }

    /// Moves a text-field caret by visual screen order. Logical UTF-8 order is
    /// not sufficient for bidi paragraphs because the next screen stop can
    /// have a smaller byte offset.
    pub fn text_field_move_horizontal(&mut self, id: ElementId, right: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let RenderKind::TextField { controller, .. } = &node.object.kind else {
            return false;
        };
        let Some(layout) = node.text_layout_cloned() else {
            return false;
        };
        if right {
            controller.move_right_visual(&layout, extend);
        } else {
            controller.move_left_visual(&layout, extend);
        }
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    pub fn text_field_move_line_edge(&mut self, id: ElementId, end: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let (
            RenderKind::TextField {
                controller,
                multiline,
                ..
            },
            Some(layout),
        ) = (&node.object.kind, node.text_layout_cloned())
        else {
            return false;
        };
        if !multiline {
            return false;
        }
        let line = line_for_byte(&layout, controller.value().selection.extent);
        let target = if end {
            layout.lines[line].caret_end
        } else {
            layout.lines[line].start
        };
        let affinity = if end {
            incular_text::TextAffinity::Upstream
        } else {
            incular_text::TextAffinity::Downstream
        };
        controller.move_cursor_with_affinity(target, affinity, extend);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    pub fn text_field_set_caret(
        &mut self,
        id: ElementId,
        point: Offset,
        extend: bool,
        now: Instant,
    ) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let (layout, scroll_x, scroll_y, multiline, controller) =
            match self.renders.get(render.0).map(|node| {
                (
                    &node.object.kind,
                    node.text_layout_cloned(),
                    node.text_field_state()
                        .expect("text-field render must own text-field state")
                        .scroll_x,
                    node.text_field_state()
                        .expect("text-field render must own text-field state")
                        .scroll_y,
                )
            }) {
                Some((
                    RenderKind::TextField {
                        controller,
                        multiline,
                        ..
                    },
                    layout,
                    scroll_x,
                    scroll_y,
                )) => (layout, scroll_x, scroll_y, *multiline, controller.clone()),
                _ => return false,
            };
        let local = self
            .render_world_transform(render)
            .inverse_transform_point(point)
            .unwrap_or(point);
        let x = local.x - 8. + scroll_x;
        let y = local.y - if multiline { 8. } else { 0. } + scroll_y;
        let text_len = controller.text().len();
        let (index, affinity) =
            layout
                .as_ref()
                .map_or((0, incular_text::TextAffinity::Downstream), |layout| {
                    let line_index = (y / layout.metrics.line_height).floor().max(0.) as usize;
                    let line_index = line_index.min(layout.lines.len().saturating_sub(1));
                    layout
                        .lines
                        .get(line_index)
                        .map_or((0, incular_text::TextAffinity::Downstream), |_| {
                            caret_for_line_position(layout, line_index, x)
                        })
                });
        let index = index.min(text_len);
        let previous = controller.value().selection;
        controller.set_selection_with_affinity(
            if extend {
                TextSelection {
                    base: previous.base,
                    extent: index,
                }
            } else {
                TextSelection::collapsed(index)
            },
            affinity,
        );
        controller.reset_caret(now);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    /// Returns the selectable label under `point`, if it belongs to a selection area.
    #[must_use]
    pub fn selectable_text_at(&self, point: Offset) -> Option<ElementId> {
        let hit = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))?;
        self.selectable_text_ancestor(hit)
    }
    #[must_use]
    pub fn is_selectable_text(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| matches!(element.widget.kind, WidgetKind::SelectableText { .. }))
    }
    /// Starts or extends read-only selection from a pointer position. The
    /// caret index is recovered from the cached Parley-backed layout; changing
    /// selection therefore never reshapes or rasterizes the text.
    pub fn selectable_text_set_selection(
        &mut self,
        id: ElementId,
        point: Offset,
        extend: bool,
    ) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(layout) = self
            .renders
            .get(render.0)
            .and_then(RenderNode::text_layout_cloned)
        else {
            return false;
        };
        let local = self
            .render_world_transform(render)
            .inverse_transform_point(point)
            .unwrap_or(point);
        let line = layout
            .lines
            .get((local.y / layout.metrics.line_height).floor().max(0.) as usize)
            .or_else(|| layout.lines.last());
        let byte = line.map_or(0, |line| caret_for_line_x(line, local.x));
        let point = StaticSelectionPoint { element: id, byte };
        if extend
            && self
                .static_selections
                .get(&area)
                .is_some_and(|selection| selection.area != area)
        {
            return false;
        }
        let anchor = if extend {
            self.static_selections
                .get(&area)
                .copied()
                .map(|selection| selection.anchor)
                .unwrap_or(point)
        } else {
            point
        };
        self.static_selections.insert(
            area,
            StaticSelection {
                area,
                anchor,
                extent: point,
            },
        );
        self.sync_static_selection(area);
        true
    }
    /// Moves the active static-text selection by one grapheme cluster. Only
    /// selection state changes; there is no editable caret or IME route.
    pub fn selectable_text_move(&mut self, id: ElementId, right: bool, extend: bool) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let Some(text) = self.selectable_text_value(id) else {
            return false;
        };
        let current = self
            .static_selections
            .get(&area)
            .copied()
            .filter(|selection| selection.extent.element == id)
            .map(|selection| selection.extent.byte)
            .unwrap_or(0);
        let byte = if right {
            next_grapheme_boundary(&text, current)
        } else {
            previous_grapheme_boundary(&text, current)
        };
        let point = StaticSelectionPoint { element: id, byte };
        let anchor = if extend {
            self.static_selections
                .get(&area)
                .copied()
                .map(|selection| selection.anchor)
                .unwrap_or(point)
        } else {
            point
        };
        self.static_selections.insert(
            area,
            StaticSelection {
                area,
                anchor,
                extent: point,
            },
        );
        self.sync_static_selection(area);
        true
    }
    pub fn selectable_text_move_to_edge(&mut self, id: ElementId, end: bool, extend: bool) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let Some(text) = self.selectable_text_value(id) else {
            return false;
        };
        let point = StaticSelectionPoint {
            element: id,
            byte: if end { text.len() } else { 0 },
        };
        let anchor = if extend {
            self.static_selections
                .get(&area)
                .copied()
                .map(|selection| selection.anchor)
                .unwrap_or(point)
        } else {
            point
        };
        self.static_selections.insert(
            area,
            StaticSelection {
                area,
                anchor,
                extent: point,
            },
        );
        self.sync_static_selection(area);
        true
    }
    pub fn selectable_text_select_all(&mut self, id: ElementId) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let entries = self.selectable_texts_in_area(area);
        let Some(first) = entries.first().copied() else {
            return false;
        };
        let Some(last) = entries.last().copied() else {
            return false;
        };
        let last_len = self
            .selectable_text_value(last)
            .map_or(0, |text| text.len());
        self.static_selections.insert(
            area,
            StaticSelection {
                area,
                anchor: StaticSelectionPoint {
                    element: first,
                    byte: 0,
                },
                extent: StaticSelectionPoint {
                    element: last,
                    byte: last_len,
                },
            },
        );
        self.sync_static_selection(area);
        true
    }
    #[must_use]
    pub fn selectable_text_selected_text(&self, id: ElementId) -> Option<String> {
        let area = self.selection_area_ancestor(id)?;
        if let Some(controller) = self.selection_area_controller(area) {
            return Some(controller.selected_text());
        }
        let selection = self.static_selections.get(&area).copied()?;
        let entries = self.selectable_texts_in_area(area);
        Some(static_selection_text(self, &entries, selection))
    }
    pub(super) fn selectable_text_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.elements.get(id.0).is_some_and(|element| {
                matches!(element.widget.kind, WidgetKind::SelectableText { .. })
            }) {
                return Some(id);
            }
            id = self.parent(id)?;
        }
    }
    pub(super) fn selection_area_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        let selectable = id;
        if !self
            .elements
            .get(id.0)
            .is_some_and(|element| matches!(element.widget.kind, WidgetKind::SelectableText { .. }))
        {
            return None;
        }
        loop {
            if let Some(policy) = self.selection_boundary_policy(id) {
                return policy.accepts_children().then_some(id);
            }
            let Some(parent) = self.parent(id) else {
                // A standalone SelectableText is its own one-label region.
                return Some(selectable);
            };
            id = parent;
        }
    }
    fn selection_boundary_policy(&self, id: ElementId) -> Option<SelectableChildPolicy> {
        match &self.elements.get(id.0)?.widget.kind {
            WidgetKind::SelectionArea { .. } => Some(SelectableChildPolicy::All),
            WidgetKind::SelectionContainer { delegate, .. }
            | WidgetKind::SelectionListener { delegate, .. } => Some(delegate.policy()),
            _ => None,
        }
    }
    pub(super) fn selection_area_controller(
        &self,
        id: ElementId,
    ) -> Option<SelectionAreaController> {
        match &self.elements.get(id.0)?.widget.kind {
            WidgetKind::SelectionArea { controller, .. } => Some(controller.clone()),
            WidgetKind::SelectionContainer { delegate, .. }
            | WidgetKind::SelectionListener { delegate, .. } => Some(delegate.controller()),
            _ => None,
        }
    }
    pub(super) fn selectable_text_value(&self, id: ElementId) -> Option<String> {
        let WidgetKind::SelectableText { text, .. } = &self.elements.get(id.0)?.widget.kind else {
            return None;
        };
        Some(text.clone())
    }
    pub(super) fn selectable_texts_in_area(&self, area: ElementId) -> Vec<ElementId> {
        let Some(policy) = self.selection_boundary_policy(area) else {
            // A standalone SelectableText owns its implicit one-item
            // selection region. It has no explicit boundary policy, but it
            // must still participate in select-all and copy operations.
            return self
                .elements
                .get(area.0)
                .is_some_and(|element| {
                    matches!(element.widget.kind, WidgetKind::SelectableText { .. })
                })
                .then_some(vec![area])
                .unwrap_or_default();
        };
        if !policy.accepts_children() {
            return Vec::new();
        }
        let mut entries = Vec::new();
        if let Some(element) = self.elements.get(area.0) {
            for child in &element.children {
                self.collect_selectable_texts(*child, &mut entries);
            }
        }
        entries
    }
    pub(super) fn collect_selectable_texts(&self, id: ElementId, entries: &mut Vec<ElementId>) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        if matches!(element.widget.kind, WidgetKind::SelectableText { .. }) {
            entries.push(id);
            return;
        }
        // A nested selection boundary owns its registrar subtree. Its leaves
        // must not be harvested by an ancestor container, even when the
        // nested boundary is disabled.
        if self.selection_boundary_policy(id).is_some() {
            return;
        }
        for child in &element.children {
            self.collect_selectable_texts(*child, entries);
        }
    }
    pub(super) fn sync_static_selection(&mut self, area: ElementId) {
        let entries = self.selectable_texts_in_area(area);
        if let Some(controller) = self.selection_area_controller(area) {
            controller.set_registered_child_count(entries.len());
            if let Some(selection) = self.static_selections.get(&area).copied() {
                let selected = static_selection_text(self, &entries, selection);
                let geometry = self.selection_geometry(area, &entries, selection);
                let range = selection_content_range(self, &entries, selection);
                controller.set_selection_snapshot(selected, range, geometry);
            } else {
                controller.clear_selection();
            }
        }
        for element in entries {
            if let Some(render) = self.render_id(element) {
                self.renders
                    .get_mut(render.0)
                    .expect("live selectable text")
                    .dirty
                    .insert(DirtyFlags::PAINT);
            }
        }
    }
    pub(super) fn static_selection_range(&self, element: ElementId) -> Option<TextRange> {
        let area = self.selection_area_ancestor(element)?;
        let selection = self.static_selections.get(&area).copied()?;
        let entries = self.selectable_texts_in_area(selection.area);
        static_selection_range(self, &entries, selection, element)
    }

    fn selection_geometry(
        &self,
        area: ElementId,
        entries: &[ElementId],
        selection: StaticSelection,
    ) -> SelectionGeometry {
        let has_content = !entries.is_empty();
        let status = if selection.anchor == selection.extent {
            SelectionStatus::Collapsed
        } else {
            SelectionStatus::Uncollapsed
        };
        let area_transform = self
            .render_id(area)
            .and_then(|render| self.render_world_transform(render).inverse())
            .unwrap_or(CoreTransform::IDENTITY);
        let mut rects = Vec::new();
        for element in entries {
            let Some(range) = static_selection_range(self, entries, selection, *element) else {
                continue;
            };
            let Some(render) = self.render_id(*element) else {
                continue;
            };
            let Some(layout) = self
                .renders
                .get(render.0)
                .and_then(RenderNode::text_layout_cloned)
            else {
                continue;
            };
            let transform = area_transform.then(self.render_world_transform(render));
            rects.extend(
                selection_rects(&layout, range, 0., 0., 0.)
                    .into_iter()
                    .map(|rect| transform.transform_rect_bbox(rect)),
            );
        }
        let start_selection_point = self.selection_point(
            area,
            entries,
            selection,
            if selection.anchor <= selection.extent {
                selection.anchor
            } else {
                selection.extent
            },
            SelectionHandleType::Left,
        );
        let end_selection_point = self.selection_point(
            area,
            entries,
            selection,
            if selection.anchor <= selection.extent {
                selection.extent
            } else {
                selection.anchor
            },
            SelectionHandleType::Right,
        );
        let (start_selection_point, end_selection_point) = if status == SelectionStatus::Collapsed {
            (
                self.selection_point(
                    area,
                    entries,
                    selection,
                    selection.anchor,
                    SelectionHandleType::Collapsed,
                ),
                None,
            )
        } else {
            (start_selection_point, end_selection_point)
        };
        SelectionGeometry {
            start_selection_point,
            end_selection_point,
            selection_rects: rects,
            status,
            has_content,
        }
    }

    fn selection_point(
        &self,
        area: ElementId,
        entries: &[ElementId],
        _selection: StaticSelection,
        point: StaticSelectionPoint,
        handle_type: SelectionHandleType,
    ) -> Option<SelectionPoint> {
        if !entries.contains(&point.element) {
            return None;
        }
        let render = self.render_id(point.element)?;
        let layout = self.renders.get(render.0)?.text_layout()?.as_ref();
        let (x, y, line_height) =
            caret_geometry(layout, point.byte, incular_text::TextAffinity::Downstream);
        let area_transform = self
            .render_id(area)
            .and_then(|render| self.render_world_transform(render).inverse())
            .unwrap_or(CoreTransform::IDENTITY);
        let position = area_transform
            .then(self.render_world_transform(render))
            .transform_point(Offset::new(x, y));
        Some(SelectionPoint {
            local_position: position,
            line_height,
            handle_type,
        })
    }

    pub(super) fn refresh_selection_states(&mut self) {
        let areas = self
            .elements
            .iter()
            .filter_map(|(raw, _element)| {
                self.selection_boundary_policy(ElementId(raw))
                    .map(|_| ElementId(raw))
                    .filter(|id| self.selection_area_controller(*id).is_some())
            })
            .collect::<Vec<_>>();
        for area in areas {
            self.sync_static_selection(area);
        }
    }
    #[must_use]
    pub fn text_controller(&self, id: ElementId) -> Option<TextEditingController> {
        let render = self.render_id(id)?;
        match &self.renders.get(render.0)?.object.kind {
            RenderKind::TextField { controller, .. } => Some(controller.clone()),
            _ => None,
        }
    }

    /// Returns the nearest [`UndoHistory`](crate::UndoHistory) capacity that
    /// wraps this text field. The value is kept on widget metadata so the
    /// renderer-neutral Widgets crate does not depend on Runtime.
    #[must_use]
    pub fn text_field_history_max_entries(&self, mut id: ElementId) -> Option<usize> {
        loop {
            let element = self.elements.get(id.0)?;
            if let Some(max_entries) = element.widget.semantics.undo_history_max_entries {
                return Some(max_entries);
            }
            id = element.parent?;
        }
    }

    /// Captures the current text-input client state after layout. The runtime
    /// turns this into a platform command; native window handles never enter
    /// the retained tree.
    #[must_use]
    pub fn text_field_input_snapshot(&self, id: ElementId) -> Option<TextFieldInputSnapshot> {
        let element = self.elements.get(id.0)?;
        let WidgetKind::TextField {
            controller,
            multiline,
            enabled,
            read_only,
            obscure_text,
            ..
        } = &element.widget.kind
        else {
            return None;
        };
        let bounds = self.element_bounds(id).unwrap_or_default();
        let caret_rect = self
            .render_id(id)
            .and_then(|render| self.renders.get(render.0).map(|node| (render, node)))
            .and_then(|(render, node)| {
                let layout = node.text_layout()?.as_ref();
                let (x, y, height) = caret_geometry(
                    layout,
                    controller.selection().extent,
                    controller.caret_affinity(),
                );
                let top = if *multiline {
                    8.0
                } else {
                    ((node.size.height - height) / 2.0).max(0.0)
                };
                Some(
                    self.render_world_transform(render).transform_rect_bbox(
                        Rect::from_origin_size(
                            Offset::new(
                                x - node
                                    .text_field_state()
                                    .expect("text-field render must own text-field state")
                                    .scroll_x
                                    + 8.0,
                                y - node
                                    .text_field_state()
                                    .expect("text-field render must own text-field state")
                                    .scroll_y
                                    + top,
                            ),
                            Size::new(1.0, height.max(1.0)),
                        ),
                    ),
                )
            })
            .unwrap_or(bounds);
        Some(TextFieldInputSnapshot {
            client_id: (u64::from(id.0.generation()) << 32) | u64::from(id.0.index()),
            text: controller.text(),
            selection: controller.selection(),
            composing: controller.composing(),
            multiline: *multiline,
            enabled: *enabled,
            read_only: *read_only,
            obscure_text: *obscure_text,
            input_type: element.widget.semantics.text_input_type.unwrap_or_default(),
            input_action: element
                .widget
                .semantics
                .text_input_action
                .unwrap_or_default(),
            bounds,
            caret_rect,
        })
    }
    pub fn submit_text_field(&self, id: ElementId) -> bool {
        let Some(element) = self.elements.get(id.0) else {
            return false;
        };
        let WidgetKind::TextField {
            controller,
            on_submit,
            ..
        } = &element.widget.kind
        else {
            return false;
        };
        if let Some(callback) = on_submit {
            callback(controller.text());
        }
        true
    }

    pub(super) fn text_field_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.is_text_field(id) {
                return Some(id);
            }
            id = self.parent(id)?;
        }
    }
}

pub(super) fn text_field_display(
    controller: &TextEditingController,
    placeholder: &str,
    obscure_text: bool,
) -> String {
    let value = controller.value();
    let mask = |text: String| {
        if obscure_text {
            // Keep one ASCII glyph per source byte.  Editing selections are
            // byte-indexed throughout the retained core, so this preserves
            // caret/selection geometry while still ensuring that the source
            // value never reaches the paint list.  For ordinary passwords
            // (the overwhelmingly common case) this is one mask glyph per
            // character; non-ASCII input remains safely obscured as well.
            "*".repeat(text.len())
        } else {
            text
        }
    };
    if let Some(preedit) = controller.preedit() {
        let range = value.selection.range();
        let mut text = value.text;
        text.replace_range(range.as_range(), &preedit);
        mask(text)
    } else if value.text.is_empty() {
        placeholder.to_owned()
    } else {
        mask(value.text.clone())
    }
}

pub(super) fn valid_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}

pub(super) fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(text)
        .take_while(|index| *index < offset)
        .last()
        .unwrap_or(0)
}

pub(super) fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(text)
        .find(|index| *index > offset)
        .unwrap_or(text.len())
}
pub(super) fn line_caret_x(line: &incular_text::TextLine, byte: usize) -> f32 {
    if byte >= line.caret_end {
        return line.width;
    }
    let mut x = 0.;
    for glyph in line.glyphs.iter() {
        if glyph.cluster as usize >= byte {
            break;
        }
        x = (glyph.offset.x + glyph.advance).max(x);
    }
    x
}
pub(super) fn line_caret_x_for_affinity(
    layout: &TextLayout,
    line: usize,
    byte: usize,
    affinity: incular_text::TextAffinity,
) -> f32 {
    layout
        .line_caret_positions(line)
        .iter()
        .find(|position| position.offset == byte && position.affinity == affinity)
        .or_else(|| {
            layout
                .line_caret_positions(line)
                .iter()
                .find(|position| position.offset == byte)
        })
        .map_or_else(
            || {
                layout
                    .lines
                    .get(line)
                    .map_or(0.0, |line| line_caret_x(line, byte))
            },
            |position| position.x,
        )
}
pub(super) fn caret_for_line_x(line: &incular_text::TextLine, x: f32) -> usize {
    if x >= line.width {
        return line.caret_end;
    }
    let mut best = 0usize;
    for glyph in line.glyphs.iter() {
        let midpoint = glyph.offset.x + glyph.advance / 2.;
        if x < midpoint {
            return glyph.cluster as usize;
        }
        best = (glyph.cluster as usize).max(best);
    }
    best.max(line.start)
}
pub(super) fn caret_for_line_position(
    layout: &TextLayout,
    line: usize,
    x: f32,
) -> (usize, incular_text::TextAffinity) {
    let Some(line_data) = layout.lines.get(line) else {
        return (0, incular_text::TextAffinity::Downstream);
    };
    let positions = layout.line_caret_positions(line);
    positions
        .iter()
        .min_by(|left, right| {
            (left.x - x)
                .abs()
                .total_cmp(&(right.x - x).abs())
                .then_with(|| left.x.total_cmp(&right.x))
        })
        .map_or_else(
            || {
                (
                    caret_for_line_x(line_data, x),
                    incular_text::TextAffinity::Downstream,
                )
            },
            |position| (position.offset, position.affinity),
        )
}
pub(super) fn line_for_byte(layout: &TextLayout, byte: usize) -> usize {
    layout
        .lines
        .iter()
        .position(|line| byte <= line.end)
        .unwrap_or_else(|| layout.lines.len().saturating_sub(1))
}
pub(super) fn caret_geometry(
    layout: &TextLayout,
    byte: usize,
    affinity: incular_text::TextAffinity,
) -> (f32, f32, f32) {
    let index = line_for_byte(layout, byte);
    let line = layout.lines.get(index);
    (
        line.map_or(0., |_| {
            line_caret_x_for_affinity(layout, index, byte, affinity)
        }),
        index as f32 * layout.metrics.line_height,
        layout.metrics.line_height,
    )
}
pub(super) fn selection_rects(
    layout: &TextLayout,
    selection: TextRange,
    scroll_x: f32,
    scroll_y: f32,
    top: f32,
) -> Vec<Rect> {
    layout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let start = selection.start.max(line.start);
            let end = selection.end.min(line.end);
            (start < end
                || (line.start == line.end
                    && selection.start <= line.start
                    && selection.end >= line.end))
                .then(|| {
                    Rect::from_origin_size(
                        Offset::new(
                            line_caret_x(line, start) - scroll_x + 8.,
                            index as f32 * layout.metrics.line_height - scroll_y + top,
                        ),
                        Size::new(
                            (line_caret_x(line, end) - line_caret_x(line, start)).max(1.),
                            layout.metrics.line_height,
                        ),
                    )
                })
        })
        .collect()
}

pub(super) fn static_selection_range(
    tree: &WidgetTree,
    entries: &[ElementId],
    selection: StaticSelection,
    element: ElementId,
) -> Option<TextRange> {
    let anchor_index = entries
        .iter()
        .position(|entry| *entry == selection.anchor.element)?;
    let extent_index = entries
        .iter()
        .position(|entry| *entry == selection.extent.element)?;
    let element_index = entries.iter().position(|entry| *entry == element)?;
    let (first_index, last_index, first_byte, last_byte) = if anchor_index <= extent_index {
        (
            anchor_index,
            extent_index,
            selection.anchor.byte,
            selection.extent.byte,
        )
    } else {
        (
            extent_index,
            anchor_index,
            selection.extent.byte,
            selection.anchor.byte,
        )
    };
    if !(first_index..=last_index).contains(&element_index) {
        return None;
    }
    let text = tree.selectable_text_value(element)?;
    let start = if element_index == first_index {
        valid_boundary(&text, first_byte)
    } else {
        0
    };
    let end = if element_index == last_index {
        valid_boundary(&text, last_byte)
    } else {
        text.len()
    };
    (start < end).then_some(TextRange::new(start, end))
}

/// Converts a retained endpoint pair into the byte range of the flattened
/// selected content. Newlines use the same one-byte separator as
/// `static_selection_text`, so listeners can use the range to address the
/// text they receive from the clipboard path.
pub(super) fn selection_content_range(
    tree: &WidgetTree,
    entries: &[ElementId],
    selection: StaticSelection,
) -> Option<SelectedContentRange> {
    fn content_offset(
        tree: &WidgetTree,
        entries: &[ElementId],
        point: StaticSelectionPoint,
    ) -> Option<usize> {
        let index = entries.iter().position(|entry| *entry == point.element)?;
        let mut offset: usize = 0;
        for entry in &entries[..index] {
            offset = offset.saturating_add(tree.selectable_text_value(*entry)?.len());
            offset = offset.saturating_add(1);
        }
        let text = tree.selectable_text_value(point.element)?;
        Some(offset.saturating_add(valid_boundary(&text, point.byte)))
    }

    let anchor = content_offset(tree, entries, selection.anchor)?;
    let extent = content_offset(tree, entries, selection.extent)?;
    Some(SelectedContentRange::new(
        anchor.min(extent),
        anchor.max(extent),
    ))
}

pub(super) fn static_selection_text(
    tree: &WidgetTree,
    entries: &[ElementId],
    selection: StaticSelection,
) -> String {
    entries
        .iter()
        .filter_map(|element| {
            let range = static_selection_range(tree, entries, selection, *element)?;
            let text = tree.selectable_text_value(*element)?;
            Some(text[range.start..range.end].to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n")
}
