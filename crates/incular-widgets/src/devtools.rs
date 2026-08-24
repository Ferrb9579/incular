//! DevTools introspection over the retained widget tree.
//!
//! Enabled only by the `devtools` feature. Everything here is read-only:
//! requesting data never marks anything dirty and never triggers rebuilds.

use crate::tree::{ElementId, InvalidationCause, Key, WidgetKind, WidgetTree};
use incular_config::Axis;
use incular_core::{Rect, Size};
use incular_devtools_protocol::{
    DebugValue, DevSemanticsId, DevWidgetId, DevWindowId, ElementState, FlexChildInspection,
    InvalidationReason, LayoutDetails, LayoutHistoryEntry, LayoutInspection, NodeDetails,
    StackChildInspection, WidgetNode, WorkReasons,
};

/// Maximum nodes serialized in one snapshot. Larger trees stream via deltas
/// after the client scrolls/requests; truncation is reported explicitly.
pub const MAX_SNAPSHOT_NODES: usize = 20_000;
/// Explicit ceiling for a whole-window geometry overlay.  The target never
/// creates unbounded debug display-list commands for pathological trees.
pub const MAX_OVERLAY_NODES: usize = 10_000;

/// Minimal target-side geometry needed by compositor-only overlay rendering.
/// It deliberately has no mutable retained IDs or widget handles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevOverlayGeometry {
    pub bounds: [f32; 4],
    pub content_bounds: Option<[f32; 4]>,
    pub baseline_y: Option<f32>,
    pub clip_bounds: Option<[f32; 4]>,
}

/// Counter snapshot used exclusively by target-side phase flashes.  It is
/// derived from existing feature-gated retained counters; no timestamps or
/// additional tree traversal occur unless a phase overlay is enabled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevPhaseNode {
    pub id: DevWidgetId,
    pub bounds: [f32; 4],
    pub builds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub semantics: u64,
    pub composites: u64,
}

impl WidgetTree {
    /// Serializes the application-facing tree under `root`. Framework
    /// plumbing widgets are excluded unless `include_internal`. Returns the
    /// depth-first node list plus a mapping used for follow-up requests.
    pub fn devtools_snapshot(
        &self,
        root: ElementId,
        include_internal: bool,
    ) -> (Vec<WidgetNode>, bool) {
        let mut nodes = Vec::new();
        let truncated = self.walk_dev(root, None, include_internal, &mut nodes, MAX_SNAPSHOT_NODES);
        (nodes, truncated)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_dev(
        &self,
        id: ElementId,
        parent: Option<DevWidgetId>,
        include_internal: bool,
        out: &mut Vec<WidgetNode>,
        max_nodes: usize,
    ) -> bool {
        if out.len() >= max_nodes {
            return true;
        }
        let Some(element) = self.dev_elements().get(id.0) else {
            return false;
        };
        // An element arena slot and generation are already a stable opaque
        // identity.  Reusing a snapshot counter here made every refresh look
        // like a completely new tree and rendered incremental deltas useless.
        let dev_id = dev_id(id);

        if !include_internal && element_is_internal(element) {
            for &child in &element.children {
                if self.walk_dev(child, parent, include_internal, out, max_nodes) {
                    return true;
                }
            }
            return false;
        }

        let child_ids = self.dev_visible_children(id, include_internal);

        out.push(WidgetNode {
            id: dev_id,
            parent,
            type_name: element.widget.kind.dev_type_name_widget(),
            key: element.widget.key.as_ref().map(key_display),
            label: leaf_label(&element.widget.kind),
            child_ids: child_ids.clone(),
            revision: dev_revision(element),
        });

        for child in self.dev_visible_child_elements(id, include_internal) {
            if self.walk_dev(child, Some(dev_id), include_internal, out, max_nodes) {
                return true;
            }
        }
        false
    }

    fn dev_visible_child_elements(&self, id: ElementId, include_internal: bool) -> Vec<ElementId> {
        let Some(element) = self.dev_elements().get(id.0) else {
            return Vec::new();
        };
        let mut visible = Vec::new();
        for &child in &element.children {
            if !include_internal
                && self
                    .dev_elements()
                    .get(child.0)
                    .is_some_and(is_element_internal)
            {
                visible.extend(self.dev_visible_child_elements(child, include_internal));
            } else {
                visible.push(child);
            }
        }
        visible
    }

    fn dev_visible_children(&self, id: ElementId, include_internal: bool) -> Vec<DevWidgetId> {
        self.dev_visible_child_elements(id, include_internal)
            .into_iter()
            .map(dev_id)
            .collect()
    }

    /// Resolves an opaque DevTools id to a currently mounted element.  The
    /// generation check rejects stale arena slots after unmount/reuse.
    pub fn devtools_resolve_id(&self, target: DevWidgetId) -> Option<ElementId> {
        self.dev_elements().iter().find_map(|(raw, _)| {
            let id = ElementId(raw);
            (dev_id(id) == target).then_some(id)
        })
    }

    /// Opaque DevTools identity for a currently mounted element.
    pub fn devtools_id_for_element(&self, id: ElementId) -> Option<DevWidgetId> {
        self.element_exists(id).then_some(dev_id(id))
    }

    /// Bounded world bounds from existing retained geometry. The caller owns
    /// the returned compact rectangles and may turn them into one diagnostic
    /// display list; this method never marks a render node dirty.
    pub fn devtools_layout_bounds(&self, limit: usize) -> Vec<[f32; 4]> {
        self.dev_elements()
            .iter()
            .filter_map(|(raw, _)| self.element_bounds(ElementId(raw)).map(rect_array))
            .take(limit.min(MAX_OVERLAY_NODES))
            .collect()
    }

    /// Bounded selected-subtree geometry. The selected node itself is
    /// included and traversal follows retained child links exactly.
    pub fn devtools_subtree_layout_bounds(&self, root: ElementId, limit: usize) -> Vec<[f32; 4]> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if out.len() == limit.min(MAX_OVERLAY_NODES) {
                break;
            }
            if let Some(bounds) = self.element_bounds(id) {
                out.push(rect_array(bounds));
            }
            if let Some(element) = self.dev_elements().get(id.0) {
                stack.extend(element.children.iter().rev().copied());
            }
        }
        out
    }

    /// Exact stored baseline and clip geometry for a selected node. Baselines
    /// are transformed through the same retained Kurbo world transform used
    /// for painting and semantics.
    pub fn devtools_overlay_geometry(&self, id: ElementId) -> Option<DevOverlayGeometry> {
        let element = self.dev_elements().get(id.0)?;
        let render = self.dev_renders().get(element.render.0)?;
        let bounds = self.element_bounds(id).map(rect_array)?;
        let (_, world, _) = self.devtools_layout_transforms(id)?;
        let baseline_y = render.baseline.map(|baseline| {
            world
                .transform_point(incular_core::Offset::new(0., baseline))
                .y
        });
        let content_bounds = if let WidgetKind::Padding { padding, .. } = &element.widget.kind {
            let local = Rect::from_origin_size(
                incular_core::Offset::new(padding.left, padding.top),
                Size::new(
                    (render.size.width - padding.horizontal()).max(0.),
                    (render.size.height - padding.vertical()).max(0.),
                ),
            );
            Some(rect_array(world.transform_rect_bbox(local)))
        } else {
            None
        };
        Some(DevOverlayGeometry {
            bounds,
            content_bounds,
            baseline_y,
            clip_bounds: render.clip_layer.map(|_| bounds),
        })
    }

    /// Existing semantic bounds, in retained reading order. Labels stay in
    /// the semantic tree and are intentionally not copied into overlay state.
    pub fn devtools_semantics_bounds(&self, limit: usize) -> Vec<[f32; 4]> {
        self.semantics()
            .iter()
            .map(|(_, node)| rect_array(node.bounds))
            .take(limit.min(MAX_OVERLAY_NODES))
            .collect()
    }

    /// Bounds of retained nodes that participate in real pointer routing.
    /// IgnorePointer regions are omitted; AbsorbPointer remains visible as it
    /// is itself the resolved hit target.
    pub fn devtools_hit_regions(&self, limit: usize) -> Vec<[f32; 4]> {
        self.dev_elements()
            .iter()
            .filter(|(_, element)| {
                matches!(
                    element.widget.kind,
                    WidgetKind::Button { .. }
                        | WidgetKind::TextField { .. }
                        | WidgetKind::SelectableText { .. }
                        | WidgetKind::SelectionArea { .. }
                        | WidgetKind::Gesture { .. }
                        | WidgetKind::Draggable { .. }
                        | WidgetKind::DragTarget { .. }
                        | WidgetKind::AbsorbPointer {
                            absorbing: true,
                            ..
                        }
                        | WidgetKind::Scroll { .. }
                        | WidgetKind::VirtualList { .. }
                )
            })
            .filter_map(|(raw, _)| self.element_bounds(ElementId(raw)).map(rect_array))
            .take(limit.min(MAX_OVERLAY_NODES))
            .collect()
    }

    /// Actual retained scroll viewport bounds. Virtual-list materialization
    /// remains visible through its existing child layout rectangles.
    pub fn devtools_scroll_viewports(&self, limit: usize) -> Vec<[f32; 4]> {
        self.dev_elements()
            .iter()
            .filter(|(_, element)| {
                matches!(
                    element.widget.kind,
                    WidgetKind::Scroll { .. } | WidgetKind::VirtualList { .. }
                )
            })
            .filter_map(|(raw, _)| self.element_bounds(ElementId(raw)).map(rect_array))
            .take(limit.min(MAX_OVERLAY_NODES))
            .collect()
    }

    /// Application-relevant retained compositor boundaries, projected via
    /// their owning elements. Internal transform plumbing is not duplicated.
    pub fn devtools_layer_bounds(&self, limit: usize) -> Vec<[f32; 4]> {
        self.dev_elements()
            .iter()
            .filter(|(raw, _)| self.devtools_is_layer_boundary(ElementId(*raw)))
            .filter_map(|(raw, _)| self.element_bounds(ElementId(raw)).map(rect_array))
            .take(limit.min(MAX_OVERLAY_NODES))
            .collect()
    }

    /// Bounded per-node counters plus retained world geometry for phase
    /// flashes. The UI samples this only while a corresponding overlay is on.
    pub fn devtools_phase_nodes(&self, limit: usize) -> Vec<DevPhaseNode> {
        self.dev_elements()
            .iter()
            .filter_map(|(raw, element)| {
                let id = ElementId(raw);
                Some(DevPhaseNode {
                    id: dev_id(id),
                    bounds: self.element_bounds(id).map(rect_array)?,
                    builds: element.dev.builds,
                    layouts: element.dev.layouts,
                    paints: element.dev.paints,
                    semantics: element.dev.semantic_updates,
                    composites: element.dev.composites,
                })
            })
            .take(limit.min(MAX_OVERLAY_NODES))
            .collect()
    }

    /// Full details for one element: counters, layout state, invalidation
    /// cause, and curated Inspectable properties.
    pub fn devtools_node_details(
        &self,
        id: ElementId,
        dev_id: DevWidgetId,
        window_id: DevWindowId,
    ) -> Option<NodeDetails> {
        let element = self.dev_elements().get(id.0)?;
        let render_node = self.dev_renders().get(element.render.0)?;
        #[cfg(feature = "devtools")]
        let properties = crate::devtools_props::inspect_properties(&element.widget.kind);
        #[cfg(not(feature = "devtools"))]
        let properties = Vec::new();

        let constraints = render_node.constraints.map(|c| DebugValue::Constraints {
            min_width: c.min_width,
            max_width: c.max_width,
            min_height: c.min_height,
            max_height: c.max_height,
        });
        let invalidation = last_invalidation(element);
        let layout = self.devtools_layout_inspection(id);

        Some(NodeDetails {
            id: dev_id,
            window: window_id,
            type_name: element.widget.kind.dev_type_name_widget(),
            key: element.widget.key.as_ref().map(key_display),
            state: ElementState {
                mount_generation: u64::from(id.0.generation()),
                builds: per_node_builds(element),
                layouts: per_node_layouts(element),
                paints: per_node_paints(element),
                composites: per_node_composites(element),
                semantic_updates: per_node_semantic_updates(element),
                constraints,
                size: Some([render_node.size.width, render_node.size.height]),
                offset: Some([render_node.offset.x, render_node.offset.y]),
                world_bounds: self.element_bounds(id).map(rect_array),
                baseline: render_node.baseline,
                clip: render_node.clip_layer.map(|_| {
                    [
                        render_node.offset.x,
                        render_node.offset.y,
                        render_node.size.width,
                        render_node.size.height,
                    ]
                }),
            },
            layout,
            layout_history: layout_history(element),
            work_reasons: WorkReasons {
                layout: element.dev.layout_reason.clone(),
                paint: element.dev.paint_reason.clone(),
                composite: element.dev.composite_reason.clone(),
            },
            properties,
            property_changes: property_changes(element),
            invalidation_causes: invalidation_causes(element),
            consumed_signals: Vec::new(),
            invalidation,
            render: Some(incular_devtools_protocol::DevRenderId::new(
                u64::from(element.render.0.index()),
                u64::from(element.render.0.generation()),
            )),
            semantics: self.dev_semantic_ids().get(&id).map(|semantic| {
                DevSemanticsId::new(
                    u64::from(semantic.0.index()),
                    u64::from(semantic.0.generation()),
                )
            }),
            source: None,
        })
    }

    /// Converts exact retained layout and compositor geometry to a compact,
    /// immutable DevTools snapshot. No layout algorithms are duplicated here:
    /// child constraints, offsets, sizes and transforms are observations from
    /// the completed pass.
    fn devtools_layout_inspection(&self, id: ElementId) -> Option<LayoutInspection> {
        let element = self.dev_elements().get(id.0)?;
        let render = self.dev_renders().get(element.render.0)?;
        let constraints = render.constraints;
        let size = render.size;
        let offset = render.offset;
        let (local, world, content) = self.devtools_layout_transforms(id)?;
        let local_transform = transform_array(local);
        let world_transform = transform_array(world);
        let world_bounds = self.element_bounds(id).map(rect_array)?;
        let padding = match &element.widget.kind {
            WidgetKind::Padding { padding, .. } => {
                Some([padding.left, padding.top, padding.right, padding.bottom])
            }
            _ => None,
        };
        let content_bounds = padding.map_or(
            [0., 0., size.width, size.height],
            |[left, top, right, bottom]| {
                [
                    left,
                    top,
                    (size.width - left - right).max(0.),
                    (size.height - top - bottom).max(0.),
                ]
            },
        );
        let details = self.devtools_layout_details(id, element, constraints, size, content);
        Some(LayoutInspection {
            node: dev_id(id),
            parent: element.parent.map(dev_id),
            incoming_constraints: constraints.map(debug_constraints),
            resolved_size: [size.width, size.height],
            local_offset: [offset.x, offset.y],
            local_transform,
            world_transform,
            world_bounds,
            content_bounds,
            padding,
            clip: render.clip_layer.map(|_| world_bounds),
            baseline: render.baseline,
            details,
        })
    }

    fn devtools_layout_details(
        &self,
        id: ElementId,
        element: &crate::tree::Element,
        constraints: Option<incular_config::Constraints>,
        size: Size,
        content_transform: incular_core::Transform,
    ) -> LayoutDetails {
        match &element.widget.kind {
            WidgetKind::Box { .. } | WidgetKind::Decorated { .. } | WidgetKind::Button { .. } => {
                LayoutDetails::Box
            }
            WidgetKind::Flex { axis, .. } => {
                self.devtools_flex_details(element, *axis, constraints, size)
            }
            WidgetKind::Stack { alignment, .. } => {
                self.devtools_stack_details(element, *alignment, None)
            }
            WidgetKind::IndexedStack {
                alignment, index, ..
            } => self.devtools_stack_details(element, *alignment, Some(*index)),
            WidgetKind::Positioned {
                left,
                right,
                top,
                bottom,
                width,
                height,
                ..
            } => LayoutDetails::Positioned {
                left: *left,
                right: *right,
                top: *top,
                bottom: *bottom,
                width: *width,
                height: *height,
            },
            WidgetKind::Transform { .. }
            | WidgetKind::Scale { .. }
            | WidgetKind::Rotation { .. } => {
                let matrix = transform_array(content_transform);
                LayoutDetails::Transform {
                    determinant: matrix[0] * matrix[3] - matrix[1] * matrix[2],
                    invertible: content_transform.inverse().is_some(),
                    matrix,
                }
            }
            WidgetKind::FittedBox { fit, alignment, .. } => {
                let matrix = transform_array(content_transform);
                let source_size = element.children.first().and_then(|child| {
                    let child = self.dev_elements().get(child.0)?;
                    let render = self.dev_renders().get(child.render.0)?;
                    Some([render.size.width, render.size.height])
                });
                LayoutDetails::Fitted {
                    fit: format!("{fit:?}"),
                    alignment: [alignment.x, alignment.y],
                    source_size,
                    destination_size: [size.width, size.height],
                    determinant: matrix[0] * matrix[3] - matrix[1] * matrix[2],
                    invertible: content_transform.inverse().is_some(),
                    matrix,
                }
            }
            WidgetKind::Scroll { controller, .. } => LayoutDetails::Scroll {
                axis: "vertical".into(),
                viewport_extent: controller.viewport_extent(),
                content_extent: controller.content_extent(),
                offset: controller.offset(),
                min_scroll: 0.,
                max_scroll: controller.max_offset(),
            },
            WidgetKind::VirtualList { .. } => {
                self.devtools_virtual_list_diagnostics(id).map_or_else(
                    || LayoutDetails::Custom {
                        layout_kind: "VirtualList (unavailable)".into(),
                    },
                    |diagnostics| LayoutDetails::LazyViewport {
                        item_count: diagnostics.logical_item_count,
                        materialized_start: diagnostics.materialized_range.start,
                        materialized_end: diagnostics.materialized_range.end,
                        materialized_items: diagnostics.materialized_item_count,
                        viewport_extent: diagnostics.viewport_extent,
                        cache_extent: diagnostics.cache_extent,
                        scroll_offset: diagnostics.scroll_offset,
                    },
                )
            }
            WidgetKind::Text {
                text,
                max_lines,
                overflow,
                ..
            } => LayoutDetails::Text {
                text_length: text.chars().count(),
                max_lines: *max_lines,
                overflow: format!("{overflow:?}"),
                line_count: self.devtools_text_line_count(id),
            },
            WidgetKind::SelectableText { text, .. } => LayoutDetails::Text {
                text_length: text.chars().count(),
                max_lines: None,
                overflow: "Visible".into(),
                line_count: self.devtools_text_line_count(id),
            },
            _ => LayoutDetails::Custom {
                layout_kind: element.widget.kind.dev_type_name_widget(),
            },
        }
    }

    fn devtools_flex_details(
        &self,
        element: &crate::tree::Element,
        axis: Axis,
        constraints: Option<incular_config::Constraints>,
        size: Size,
    ) -> LayoutDetails {
        let children = element
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, child_id)| {
                let child = self.dev_elements().get(child_id.0)?;
                let render = self.dev_renders().get(child.render.0)?;
                let (flex, fit) = match &child.widget.kind {
                    WidgetKind::Flexible { flex, fit, .. } => {
                        (Some(*flex), Some(format!("{fit:?}")))
                    }
                    _ => (None, None),
                };
                let allocated_main_extent = render.constraints.map(|constraints| match axis {
                    Axis::Horizontal => constraints.max_width,
                    Axis::Vertical => constraints.max_height,
                });
                Some(FlexChildInspection {
                    node: dev_id(*child_id),
                    index,
                    type_name: child.widget.kind.dev_type_name_widget(),
                    size: [render.size.width, render.size.height],
                    offset: [render.offset.x, render.offset.y],
                    flex,
                    fit,
                    allocated_main_extent,
                    actual_main_extent: axis.main_extent(render.size),
                    cross_extent: axis.cross_extent(render.size),
                })
            })
            .collect::<Vec<_>>();
        let non_flex_extent = children
            .iter()
            .filter(|child| child.flex.is_none())
            .map(|child| child.actual_main_extent)
            .sum();
        let flexible_extent = children
            .iter()
            .filter(|child| child.flex.is_some())
            .map(|child| child.actual_main_extent)
            .sum();
        let used_extent = non_flex_extent + flexible_extent;
        let available_main = constraints.and_then(|constraints| {
            let value = match axis {
                Axis::Horizontal => constraints.max_width,
                Axis::Vertical => constraints.max_height,
            };
            value.is_finite().then_some(value)
        });
        let resolved_main = axis.main_extent(size);
        LayoutDetails::Flex {
            axis: if axis.is_horizontal() {
                "horizontal"
            } else {
                "vertical"
            }
            .into(),
            available_main,
            non_flex_extent,
            flexible_extent,
            used_extent,
            remaining_extent: (resolved_main - used_extent).max(0.),
            overflow: (used_extent - resolved_main).max(0.),
            children,
        }
    }

    fn devtools_stack_details(
        &self,
        element: &crate::tree::Element,
        alignment: incular_config::Alignment,
        indexed_active: Option<usize>,
    ) -> LayoutDetails {
        let children = element
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, child_id)| {
                let child = self.dev_elements().get(child_id.0)?;
                let render = self.dev_renders().get(child.render.0)?;
                let (left, right, top, bottom, width, height) = match &child.widget.kind {
                    WidgetKind::Positioned {
                        left,
                        right,
                        top,
                        bottom,
                        width,
                        height,
                        ..
                    } => (*left, *right, *top, *bottom, *width, *height),
                    _ => (None, None, None, None, None, None),
                };
                Some(StackChildInspection {
                    node: dev_id(*child_id),
                    index,
                    type_name: child.widget.kind.dev_type_name_widget(),
                    bounds: [
                        render.offset.x,
                        render.offset.y,
                        render.size.width,
                        render.size.height,
                    ],
                    painted: indexed_active.is_none_or(|active| active == index),
                    left,
                    right,
                    top,
                    bottom,
                    width,
                    height,
                })
            })
            .collect();
        LayoutDetails::Stack {
            alignment: [alignment.x, alignment.y],
            indexed_active,
            children,
        }
    }

    /// Human-readable ancestor path for signal-subscriber rows.
    pub fn devtools_element_path(&self, id: ElementId) -> String {
        let mut parts = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            match self.dev_elements().get(current.0) {
                Some(element) => {
                    parts.push(element.widget.kind.dev_type_name_widget());
                    cursor = element.parent;
                }
                None => break,
            }
        }
        parts.reverse();
        parts.join(" / ")
    }

    /// Deepest application-facing element at `point` for Select Widget mode.
    pub fn devtools_deepest_at(&self, point: incular_core::Offset) -> Option<ElementId> {
        let render = self.hit_test(point)?;
        self.element_for_render(render)
    }
}

fn debug_constraints(constraints: incular_config::Constraints) -> DebugValue {
    DebugValue::Constraints {
        min_width: constraints.min_width,
        max_width: constraints.max_width,
        min_height: constraints.min_height,
        max_height: constraints.max_height,
    }
}

fn rect_array(rect: Rect) -> [f32; 4] {
    [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ]
}

fn transform_array(transform: incular_core::Transform) -> [f32; 6] {
    let coefficients = transform.to_kurbo().as_coeffs();
    [
        coefficients[0] as f32,
        coefficients[1] as f32,
        coefficients[2] as f32,
        coefficients[3] as f32,
        coefficients[4] as f32,
        coefficients[5] as f32,
    ]
}

#[cfg(feature = "devtools")]
fn property_changes(
    element: &crate::tree::Element,
) -> Vec<incular_devtools_protocol::PropertyChange> {
    element.dev.property_changes.clone()
}

#[cfg(not(feature = "devtools"))]
fn property_changes(
    _element: &crate::tree::Element,
) -> Vec<incular_devtools_protocol::PropertyChange> {
    Vec::new()
}

fn dev_id(id: ElementId) -> DevWidgetId {
    DevWidgetId::new(u64::from(id.0.index()), u64::from(id.0.generation()))
}

fn per_node_builds(element: &crate::tree::Element) -> u64 {
    #[cfg(feature = "devtools")]
    {
        element.dev.builds
    }
    #[cfg(not(feature = "devtools"))]
    {
        let _ = element;
        0
    }
}
fn per_node_layouts(element: &crate::tree::Element) -> u64 {
    #[cfg(feature = "devtools")]
    {
        element.dev.layouts
    }
    #[cfg(not(feature = "devtools"))]
    {
        let _ = element;
        0
    }
}
fn per_node_paints(element: &crate::tree::Element) -> u64 {
    #[cfg(feature = "devtools")]
    {
        element.dev.paints
    }
    #[cfg(not(feature = "devtools"))]
    {
        let _ = element;
        0
    }
}
fn per_node_composites(element: &crate::tree::Element) -> u64 {
    #[cfg(feature = "devtools")]
    {
        element.dev.composites
    }
    #[cfg(not(feature = "devtools"))]
    {
        let _ = element;
        0
    }
}
fn per_node_semantic_updates(element: &crate::tree::Element) -> u64 {
    #[cfg(feature = "devtools")]
    {
        element.dev.semantic_updates
    }
    #[cfg(not(feature = "devtools"))]
    {
        let _ = element;
        0
    }
}

#[cfg(feature = "devtools")]
fn layout_history(element: &crate::tree::Element) -> Vec<LayoutHistoryEntry> {
    element
        .dev
        .layout_history
        .iter()
        .map(|entry| LayoutHistoryEntry {
            sequence: entry.sequence,
            old_constraints: entry.old_constraints.map(debug_constraints),
            new_constraints: debug_constraints(entry.new_constraints),
            old_size: [entry.old_size.width, entry.old_size.height],
            new_size: [entry.new_size.width, entry.new_size.height],
            cause: entry.cause.clone(),
        })
        .collect()
}

#[cfg(not(feature = "devtools"))]
fn layout_history(_element: &crate::tree::Element) -> Vec<LayoutHistoryEntry> {
    Vec::new()
}

#[cfg(feature = "devtools")]
fn last_invalidation(element: &crate::tree::Element) -> Option<InvalidationReason> {
    element.dev.last_cause.as_ref().map(map_cause)
}

#[cfg(not(feature = "devtools"))]
fn last_invalidation(_element: &crate::tree::Element) -> Option<InvalidationReason> {
    None
}

#[cfg(feature = "devtools")]
fn invalidation_causes(element: &crate::tree::Element) -> Vec<InvalidationReason> {
    element
        .dev
        .invalidation_causes
        .iter()
        .map(map_cause)
        .collect()
}

#[cfg(not(feature = "devtools"))]
fn invalidation_causes(_element: &crate::tree::Element) -> Vec<InvalidationReason> {
    Vec::new()
}

#[cfg_attr(not(feature = "devtools"), allow(dead_code))]
pub(crate) fn map_cause(cause: &InvalidationCause) -> InvalidationReason {
    match cause {
        InvalidationCause::Signal { id, name, old, new } => InvalidationReason::SignalWrite {
            signal: incular_devtools_protocol::DevSignalId::new(*id, 1),
            name: name.clone().unwrap_or_default(),
            old: old.clone(),
            new: new.clone(),
        },
        InvalidationCause::ParentReconciliation => InvalidationReason::ParentReconciliation {
            changed: Vec::new(),
        },
        InvalidationCause::WidgetConfigurationChanged => {
            InvalidationReason::WidgetConfigurationChanged
        }
        InvalidationCause::EnvironmentChanged => InvalidationReason::EnvironmentChanged {
            field: String::new(),
            old: None,
            new: None,
        },
        InvalidationCause::LocaleChanged => InvalidationReason::LocaleChanged,
        InvalidationCause::WindowMetricsChanged => InvalidationReason::WindowMetricsChanged,
        InvalidationCause::ConstraintsChanged => InvalidationReason::ConstraintsChanged,
        InvalidationCause::Animation => InvalidationReason::Animation,
        InvalidationCause::TaskCompletion => InvalidationReason::TaskCompletion,
        InvalidationCause::Navigation => InvalidationReason::Navigation,
        InvalidationCause::Restoration => InvalidationReason::Restoration,
        InvalidationCause::Manual => InvalidationReason::ManualInvalidation,
        InvalidationCause::Mounted => InvalidationReason::Mounted,
    }
}

fn dev_revision(element: &crate::tree::Element) -> u64 {
    #[cfg(feature = "devtools")]
    {
        element.dev.revision
    }
    #[cfg(not(feature = "devtools"))]
    {
        let _ = element;
        0
    }
}

fn is_element_internal(element: &crate::tree::Element) -> bool {
    matches!(
        element.widget.kind,
        WidgetKind::RepaintBoundary { .. } | WidgetKind::LayoutBuilder { .. }
    )
}

fn element_is_internal(element: &crate::tree::Element) -> bool {
    is_element_internal(element)
}

fn key_display(key: &Key) -> String {
    match key {
        Key::Value(value) => format!("<{value}>"),
        Key::String(text) => text.clone(),
    }
}

/// Truncated leaf labels. Editable field content is privacy-sensitive and
/// never leaves the target; only neutral summaries cross the wire.
fn leaf_label(kind: &WidgetKind) -> Option<String> {
    match kind {
        WidgetKind::Text { text, .. } | WidgetKind::SelectableText { text, .. } => {
            Some(truncate(text, 48))
        }
        WidgetKind::Button { child, .. } => child
            .as_ref()
            .and_then(|child| child.text_if_any().map(|text| truncate(&text, 32))),
        WidgetKind::TextField {
            placeholder,
            multiline,
            ..
        } => {
            let kind_word = if *multiline {
                "multiline field"
            } else {
                "field"
            };
            Some(if placeholder.is_empty() {
                kind_word.to_owned()
            } else {
                format!("{} ({})", kind_word, truncate(placeholder, 24))
            })
        }
        _ => None,
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(limit).collect();
        out.push('…');
        out
    }
}

#[allow(dead_code)]
fn unused_size_marker(_size: Size) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{TranslationController, Widget};
    use incular_config::{Constraints, FlexFit};
    use incular_core::{Color, Offset};
    use incular_devtools_protocol::{DebugValue, TracePhase};
    use std::time::{Duration, Instant};

    #[test]
    fn snapshots_keep_arena_ids_stable_between_polls() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::column(vec![
                Widget::text("first"),
                Widget::text("second"),
            ]))
            .expect("mount");
        let (first, _) = tree.devtools_snapshot(root, false);
        let (second, _) = tree.devtools_snapshot(root, false);
        let first_ids: Vec<_> = first.iter().map(|node| node.id).collect();
        let second_ids: Vec<_> = second.iter().map(|node| node.id).collect();
        assert_eq!(first_ids, second_ids);
        assert!(
            first
                .iter()
                .all(|node| tree.devtools_resolve_id(node.id).is_some())
        );
    }

    #[test]
    fn editable_opacity_override_is_typed_and_visible_in_details() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::opacity(
                0.75,
                Widget::box_(Size::new(10., 10.), Color::WHITE),
            ))
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(10., 10.)));
        let id = dev_id(root);
        let before = tree
            .devtools_node_details(root, id, DevWindowId::new(1, 0))
            .expect("details");
        assert!(before.properties.iter().any(|property| {
            property.name == "opacity"
                && property.editable
                && property.value == DebugValue::Float(0.75)
        }));

        assert!(tree.devtools_edit_property(id, "opacity", &DebugValue::Float(0.25)));
        let after = tree
            .devtools_node_details(root, id, DevWindowId::new(1, 0))
            .expect("details");
        assert!(after.properties.iter().any(|property| {
            property.name == "opacity" && property.value == DebugValue::Float(0.25)
        }));
        assert!(!tree.devtools_edit_property(id, "opacity", &DebugValue::Str("invalid".into())));
    }

    #[test]
    fn flex_inspection_uses_retained_child_constraints_and_offsets() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::row(vec![
                Widget::box_(Size::new(20., 10.), Color::WHITE),
                Widget::flexible(
                    1,
                    FlexFit::Tight,
                    Widget::box_(Size::new(4., 10.), Color::WHITE),
                ),
            ]))
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(100., 30.)));
        let details = tree
            .devtools_node_details(root, dev_id(root), DevWindowId::new(1, 1))
            .expect("details");
        let Some(layout) = details.layout else {
            panic!("layout snapshot");
        };
        let LayoutDetails::Flex {
            children, overflow, ..
        } = layout.details
        else {
            panic!("flex inspection");
        };
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].actual_main_extent, 20.);
        assert_eq!(children[1].offset, [20., 0.]);
        assert_eq!(children[1].allocated_main_extent, Some(80.));
        assert_eq!(children[1].actual_main_extent, 80.);
        assert_eq!(overflow, 0.);
    }

    #[test]
    fn whole_tree_overlay_snapshot_has_a_hard_bound() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::column(
                (0..32)
                    .map(|_| Widget::box_(Size::new(1., 1.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ))
            .expect("mount");
        tree.layout(Constraints::tight(Size::new(10., 40.)));
        assert_eq!(tree.devtools_layout_bounds(3).len(), 3);
        assert!(tree.devtools_subtree_layout_bounds(root, 2).len() <= 2);
    }

    #[test]
    fn coalesced_invalidation_snapshot_keeps_multiple_real_causes_bounded() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Widget::text("cause")).expect("mount");
        for _ in 0..10 {
            tree.note_invalidation(root, InvalidationCause::Manual);
        }
        let details = tree
            .devtools_node_details(root, dev_id(root), DevWindowId::new(1, 1))
            .expect("details");
        assert_eq!(details.invalidation_causes.len(), 8);
        assert!(matches!(
            details.invalidation,
            Some(InvalidationReason::ManualInvalidation)
        ));
    }

    #[test]
    fn animation_time_scale_changes_only_retained_animation_progression() {
        let controller = TranslationController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::translate(
            controller.clone(),
            Widget::box_(Size::new(10., 10.), Color::WHITE),
        ))
        .expect("mount");
        tree.layout(Constraints::tight(Size::new(20., 20.)));
        let origin = Instant::now();
        tree.set_animation_time_scale(0.5);
        let _ = tree.update_compositor(origin);
        controller.animate_to(Offset::new(100., 0.), Duration::from_millis(100), origin);
        let _ = tree.update_compositor(origin + Duration::from_millis(16));
        assert!((controller.offset().x - 8.).abs() < 0.1);

        tree.set_animation_time_scale(0.);
        let _ = tree.update_compositor(origin + Duration::from_millis(32));
        assert!((controller.offset().x - 8.).abs() < 0.1);
    }

    #[test]
    fn phase_snapshot_uses_existing_work_counters_and_respects_limit() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Widget::text("phase")).expect("mount");
        tree.layout(Constraints::tight(Size::new(50., 20.)));
        let _ = tree.paint();
        let phase = tree.devtools_phase_nodes(1);
        assert_eq!(phase.len(), 1);
        assert_eq!(phase[0].id, dev_id(root));
        assert!(phase[0].layouts > 0);
        assert!(phase[0].paints > 0);
    }

    #[test]
    fn deep_trace_is_opt_in_and_layout_events_preserve_parentage() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::column(vec![Widget::box_(
            Size::new(10., 10.),
            Color::WHITE,
        )]))
        .expect("mount");
        tree.layout(Constraints::tight(Size::new(20., 20.)));
        assert!(tree.take_deep_trace().is_none());

        tree.begin_deep_trace(32);
        tree.layout(Constraints::tight(Size::new(24., 24.)));
        let (events, dropped) = tree.take_deep_trace().expect("deep trace");
        let layouts = events
            .iter()
            .filter(|event| event.phase == TracePhase::Layout)
            .collect::<Vec<_>>();
        assert!(layouts.len() >= 2);
        assert_eq!(layouts[0].parent, None);
        assert_eq!(layouts[1].parent, Some(0));
        assert_eq!(dropped, 0);
        let root = tree.root().expect("root");
        let details = tree
            .devtools_node_details(root, dev_id(root), DevWindowId::new(1, 0))
            .expect("details");
        assert!(!details.layout_history.is_empty());
    }

    #[test]
    fn deep_trace_event_buffer_reports_truncation_inputs() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::column(
            (0..8)
                .map(|_| Widget::box_(Size::new(1., 1.), Color::WHITE))
                .collect::<Vec<_>>(),
        ))
        .expect("mount");
        tree.begin_deep_trace(2);
        tree.layout(Constraints::tight(Size::new(10., 20.)));
        let (events, dropped) = tree.take_deep_trace().expect("trace");
        assert_eq!(events.len(), 2);
        assert!(dropped > 0);
    }

    #[test]
    fn auxiliary_overlays_read_retained_subsystems_and_stay_bounded() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            crate::ScrollController::new(),
            Widget::button(Size::new(30., 20.), Color::WHITE, crate::ActionId(1)),
        ))
        .expect("mount");
        tree.layout(Constraints::tight(Size::new(40., 30.)));
        tree.update_semantics();
        let before = tree.diagnostics();
        assert_eq!(tree.devtools_scroll_viewports(1).len(), 1);
        assert!(!tree.devtools_hit_regions(8).is_empty());
        assert!(!tree.devtools_semantics_bounds(8).is_empty());
        assert!(!tree.devtools_layer_bounds(8).is_empty());
        assert!(tree.devtools_hit_regions(1).len() <= 1);
        assert_eq!(tree.diagnostics(), before);
    }
}
