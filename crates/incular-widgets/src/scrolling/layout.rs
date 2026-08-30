use super::*;

pub(super) struct BoxRenderSliver {
    pub(super) child: Widget,
    pub(super) extent: Cell<f32>,
    pub(super) pinned: bool,
}

/// Geometry shared by the pinned persistent-header variants.
///
/// A pinned header has a normal scroll extent, but its paint extent continues
/// at the leading edge after its layout extent has collapsed.  Keeping that
/// distinction in the sliver geometry is what lets the viewport compute the
/// same obstruction/overlap values as Flutter's persistent-header render
/// objects instead of relying only on a post-layout position adjustment.
fn pinned_geometry(
    constraints: SliverConstraints,
    scroll_extent: f32,
    child_extent: f32,
) -> SliverGeometry {
    let scroll_extent = scroll_extent.max(0.);
    let child_extent = child_extent.max(0.);
    let effective_remaining_paint_extent =
        (constraints.remaining_paint_extent - constraints.overlap).max(0.);
    let paint_extent = child_extent.min(effective_remaining_paint_extent);
    let layout_extent =
        (scroll_extent - constraints.scroll_offset).clamp(0., effective_remaining_paint_extent);
    let cache_extent = if layout_extent > 0. {
        (-constraints.cache_origin + layout_extent).max(0.)
    } else {
        layout_extent
    };
    SliverGeometry {
        scroll_extent,
        paint_extent,
        layout_extent,
        max_paint_extent: scroll_extent,
        hit_test_extent: paint_extent,
        paint_origin: constraints.overlap,
        cache_extent,
        visible: paint_extent > 0.,
        has_visual_overflow: true,
        scroll_offset_correction: None,
    }
}

impl BoxRenderSliver {
    pub(super) fn new(child: Widget) -> Self {
        Self {
            child,
            extent: Cell::new(DEFAULT_LAZY_ITEM_EXTENT),
            pinned: false,
        }
    }

    pub(super) fn pinned(child: Widget) -> Self {
        Self {
            pinned: true,
            ..Self::new(child)
        }
    }
}

impl RenderSliver for BoxRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.extent.get().max(0.);
        let geometry = if self.pinned {
            pinned_geometry(constraints, extent, extent)
        } else {
            SliverGeometry::from_scroll_extent(constraints, extent)
        };
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    None,
                ),
                extent,
                pinned: self.pinned,
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let extent = extent.max(0.);
        if (self.extent.get() - extent).abs() <= f32::EPSILON {
            return false;
        }
        self.extent.set(extent);
        true
    }
}

pub(super) struct FixedExtentRenderSliver {
    pub(super) item_count: usize,
    pub(super) item_extent: f32,
    pub(super) builder: Rc<dyn Fn(usize) -> Widget>,
    pub(super) widgets: HashMap<usize, Widget>,
}

impl FixedExtentRenderSliver {
    pub(super) fn new(
        item_count: usize,
        item_extent: f32,
        builder: Rc<dyn Fn(usize) -> Widget>,
    ) -> Self {
        Self {
            item_count,
            item_extent: item_extent.max(1.),
            builder,
            widgets: HashMap::new(),
        }
    }

    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = (self.builder)(index);
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for FixedExtentRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.item_count)
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let total = self.item_count as f32 * self.item_extent;
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let start = (cache_start / self.item_extent).floor() as usize;
        let end = (cache_end / self.item_extent).ceil() as usize;
        let range =
            start.min(self.item_count)..end.min(self.item_count).max(start.min(self.item_count));
        self.widgets.retain(|index, _| range.contains(index));
        let children = range
            .map(|index| {
                let widget = self.child_widget(index);
                SliverChildLayout {
                    id: SliverChildId::list_item(index),
                    widget,
                    semantic_index: Some(index),
                    offset: index as f32 * self.item_extent,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(self.item_extent),
                    ),
                    extent: self.item_extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, total),
            children,
            absorbed_overlap: 0.,
        }
    }
}

/// Retained grid layout driven by a [`SliverGridDelegate`]. The grid only
/// materializes rows/columns intersecting the paint and cache intervals, and
/// recomputes tile dimensions from the current sliver constraints.
pub(super) struct GridRenderSliver {
    pub(super) item_count: usize,
    pub(super) delegate: SliverGridDelegate,
    pub(super) builder: Rc<dyn Fn(usize) -> Widget>,
    pub(super) axis: Axis,
    pub(super) widgets: HashMap<usize, Widget>,
}

impl GridRenderSliver {
    pub(super) fn new(
        item_count: usize,
        delegate: SliverGridDelegate,
        builder: Rc<dyn Fn(usize) -> Widget>,
        axis: Axis,
    ) -> Self {
        Self {
            item_count,
            delegate,
            builder,
            axis,
            widgets: HashMap::new(),
        }
    }

    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = (self.builder)(index);
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for GridRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.item_count)
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let cross_extent = constraints.cross_axis_extent.max(0.);
        let columns = self.delegate.cross_axis_count(cross_extent).max(1);
        let main_extent = self
            .delegate
            .resolve_main_axis_extent(cross_extent, columns);
        let main_spacing = self.delegate.resolved_main_axis_spacing();
        let cross_spacing = self.delegate.resolved_cross_axis_spacing();
        let rows = self.item_count.div_ceil(columns);
        let row_step = (main_extent + main_spacing).max(1.);
        let total = if rows == 0 {
            0.
        } else {
            rows as f32 * main_extent + rows.saturating_sub(1) as f32 * main_spacing
        };
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let start_row = (cache_start / row_step).floor() as usize;
        let end_row = (cache_end / row_step).ceil() as usize;
        let row_range = start_row.min(rows)..end_row.min(rows).max(start_row.min(rows));
        let first_item = row_range.start.saturating_mul(columns).min(self.item_count);
        let last_item = row_range.end.saturating_mul(columns).min(self.item_count);
        self.widgets
            .retain(|index, _| (*index >= first_item) && (*index < last_item));

        let cell_cross_extent = if columns == 0 {
            0.
        } else {
            (cross_extent - cross_spacing * columns.saturating_sub(1) as f32).max(0.)
                / columns as f32
        };
        let axis = self.axis;
        let children = row_range
            .map(|row| {
                let start = row * columns;
                let end = (start + columns).min(self.item_count);
                let cells = (start..end).map(|index| -> Widget {
                    let child = self.child_widget(index);
                    if axis == Axis::Vertical {
                        Widget::from(
                            SizedBox::new()
                                .width(cell_cross_extent)
                                .height(main_extent)
                                .child(child),
                        )
                    } else {
                        Widget::from(
                            SizedBox::new()
                                .width(main_extent)
                                .height(cell_cross_extent)
                                .child(child),
                        )
                    }
                });
                let widget = if axis == Axis::Vertical {
                    Widget::from(Row::new(cells).spacing(cross_spacing))
                } else {
                    Widget::from(Column::new(cells).spacing(cross_spacing))
                };
                SliverChildLayout {
                    id: SliverChildId::list_item(row),
                    widget,
                    semantic_index: Some(row),
                    offset: row as f32 * row_step,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(main_extent),
                    ),
                    extent: main_extent,
                    pinned: false,
                }
            })
            .collect();

        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, total),
            children,
            absorbed_overlap: 0.,
        }
    }
}

pub(super) struct VariableExtentRenderSliver {
    pub(super) index: MeasuredExtentIndex,
    pub(super) builder: Rc<dyn Fn(usize) -> Widget>,
    pub(super) widgets: HashMap<usize, Widget>,
}

impl VariableExtentRenderSliver {
    pub(super) fn new(index: MeasuredExtentIndex, builder: Rc<dyn Fn(usize) -> Widget>) -> Self {
        Self {
            index,
            builder,
            widgets: HashMap::new(),
        }
    }

    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = (self.builder)(index);
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for VariableExtentRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.index.len())
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let range =
            self.index
                .materialized_range(cache_start, (cache_end - cache_start).max(0.), 0.);
        self.widgets.retain(|index, _| range.contains(index));
        let children = range
            .map(|index| {
                let widget = self.child_widget(index);
                SliverChildLayout {
                    id: SliverChildId::list_item(index),
                    widget,
                    semantic_index: Some(index),
                    offset: self.index.offset_for_index(index),
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        None,
                    ),
                    extent: self.index.offset_for_index(index + 1)
                        - self.index.offset_for_index(index),
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, self.index.total_extent()),
            children,
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        child
            .0
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .is_some_and(|index| self.index.set_measured_extent(index, extent))
    }

    fn revision(&self) -> u64 {
        self.index.revision()
    }
}

pub(super) struct HeaderRenderSliver {
    pub(super) child: Widget,
    pub(super) extent: f32,
    pub(super) pinned: bool,
}

impl HeaderRenderSliver {
    pub(super) fn new(child: Widget, extent: f32, pinned: bool) -> Self {
        Self {
            child,
            extent: extent.max(0.),
            pinned,
        }
    }
}

/// Retained floating-header state. A floating header follows the normal
/// scroll offset while moving forward, but reveals by the same delta when the
/// viewport starts moving back toward the leading edge.
pub(super) struct FloatingHeaderRenderSliver {
    pub(super) child: Widget,
    pub(super) extent: Cell<f32>,
    pub(super) last_scroll_offset: Option<f32>,
    pub(super) effective_scroll_offset: f32,
}

impl RenderSliver for FloatingHeaderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.extent.get().max(0.);
        let scroll_offset = constraints.scroll_offset.max(0.);
        if let Some(previous) = self.last_scroll_offset {
            self.effective_scroll_offset = (self.effective_scroll_offset
                + (scroll_offset - previous))
                .clamp(0., scroll_offset.max(extent));
        } else {
            self.effective_scroll_offset = scroll_offset;
        }
        self.last_scroll_offset = Some(scroll_offset);

        let effective_remaining_paint_extent =
            (constraints.remaining_paint_extent - constraints.overlap).max(0.);
        let paint_extent = (extent - self.effective_scroll_offset)
            .max(0.)
            .min(effective_remaining_paint_extent);
        let layout_extent = (extent - scroll_offset)
            .clamp(0., effective_remaining_paint_extent)
            .min(paint_extent);
        let geometry = SliverGeometry {
            scroll_extent: extent,
            paint_extent,
            layout_extent,
            max_paint_extent: extent,
            hit_test_extent: paint_extent,
            paint_origin: constraints.overlap.min(0.),
            cache_extent: if layout_extent > 0. {
                (-constraints.cache_origin + layout_extent).max(0.)
            } else {
                0.
            },
            visible: paint_extent > 0.,
            has_visual_overflow: true,
            scroll_offset_correction: None,
        };
        SliverLayout {
            geometry,
            // The sequence converts this back through the viewport transform.
            // `extent - effective` is not a normal flow offset: it exposes
            // the child at the leading edge while the header is floating.
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset: scroll_offset - self.effective_scroll_offset,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(extent),
                ),
                extent,
                pinned: false,
            }],
            absorbed_overlap: (paint_extent - layout_extent).max(0.),
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let extent = extent.max(0.);
        if (self.extent.get() - extent).abs() <= f32::EPSILON {
            return false;
        }
        self.extent.set(extent);
        true
    }
}

impl RenderSliver for HeaderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let geometry = if self.pinned {
            pinned_geometry(constraints, self.extent, self.extent)
        } else {
            SliverGeometry::from_scroll_extent(constraints, self.extent)
        };
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(self.extent),
                ),
                extent: self.extent,
                pinned: self.pinned,
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }
}

pub(super) struct ResizingHeaderRenderSliver {
    pub(super) child: Widget,
    pub(super) min_extent: f32,
    pub(super) max_extent: f32,
}

pub(super) struct FillRemainingRenderSliver {
    pub(super) child: Widget,
    pub(super) has_scroll_body: bool,
    pub(super) extent: Cell<f32>,
}

impl RenderSliver for FillRemainingRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let child_hint = widget_main_extent_hint(&self.child, constraints.axis).unwrap_or(0.);
        let remaining_extent =
            (constraints.viewport_main_axis_extent - constraints.preceding_scroll_extent).max(0.);
        let extent = if self.has_scroll_body {
            (constraints.remaining_paint_extent - constraints.overlap.min(0.)).max(0.)
        } else {
            self.extent.get().max(remaining_extent).max(child_hint)
        };
        self.extent.set(extent);
        let scroll_extent = if self.has_scroll_body {
            constraints.viewport_main_axis_extent.max(0.)
        } else {
            extent
        };
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, scroll_extent),
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                // The parent sequence contributes the preceding scroll
                // extent; a sliver child is always positioned in this
                // sliver's local coordinate space.
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(extent),
                ),
                extent,
                pinned: false,
            }],
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let changed = (self.extent.get() - extent).abs() > f32::EPSILON;
        self.extent.set(extent);
        changed
    }
}

pub(super) struct ViewportExtentRenderSliver {
    pub(super) item_count: usize,
    pub(super) children: Option<Rc<Vec<Widget>>>,
    pub(super) builder: Option<Rc<dyn Fn(usize) -> Widget>>,
    pub(super) widgets: HashMap<usize, Widget>,
    pub(super) viewport_fraction: f32,
    pub(super) fallback_extent: f32,
}

impl ViewportExtentRenderSliver {
    fn child_widget(&mut self, index: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&index) {
            return widget.clone();
        }
        let widget = self
            .children
            .as_ref()
            .and_then(|children| children.get(index).cloned())
            .or_else(|| self.builder.as_ref().map(|builder| builder(index)))
            .unwrap_or_else(|| Widget::from(SizedBox::shrink()));
        self.widgets.insert(index, widget.clone());
        widget
    }
}

impl RenderSliver for ViewportExtentRenderSliver {
    fn child_count(&self) -> Option<usize> {
        Some(self.item_count)
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let viewport_extent = if constraints.viewport_main_axis_extent.is_finite()
            && constraints.viewport_main_axis_extent > 0.
        {
            constraints.viewport_main_axis_extent
        } else {
            self.fallback_extent
        };
        let extent = (viewport_extent * self.viewport_fraction).max(0.);
        let total = self.item_count as f32 * extent;
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let start = if extent > 0. {
            (cache_start / extent).floor() as usize
        } else {
            0
        };
        let end = if extent > 0. {
            (cache_end / extent).ceil() as usize
        } else {
            0
        };
        let range =
            start.min(self.item_count)..end.min(self.item_count).max(start.min(self.item_count));
        self.widgets.retain(|index, _| range.contains(index));
        let children = range
            .map(|index| {
                let widget = self.child_widget(index);
                SliverChildLayout {
                    id: SliverChildId::list_item(index),
                    widget,
                    semantic_index: Some(index),
                    offset: index as f32 * extent,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(extent),
                    ),
                    extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, total),
            children,
            absorbed_overlap: 0.,
        }
    }
}

impl RenderSliver for ResizingHeaderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let current =
            (self.max_extent - constraints.scroll_offset).clamp(self.min_extent, self.max_extent);
        let geometry = pinned_geometry(constraints, self.max_extent, current);
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(current),
                ),
                extent: current,
                pinned: true,
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }
}

pub(super) struct PaddingRenderSliver {
    pub(super) inner: RefCell<Box<dyn RenderSliver>>,
    pub(super) padding: EdgeInsets,
}

impl RenderSliver for PaddingRenderSliver {
    fn child_count(&self) -> Option<usize> {
        self.inner.borrow().child_count()
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let before = main_before(constraints.axis, self.padding);
        let after = main_after(constraints.axis, self.padding);
        let total_padding = before + after;
        let paint_offset = |from: f32, to: f32| {
            let start = from.max(constraints.scroll_offset);
            let end = to.min(constraints.scroll_offset + constraints.remaining_paint_extent);
            (end - start).max(0.)
        };
        let cache_offset = |from: f32, to: f32| {
            let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
            let start = from.max(cache_start);
            let end = to.min(cache_start + constraints.remaining_cache_extent);
            (end - start).max(0.)
        };
        let reduced_cross = (constraints.cross_axis_extent
            - cross_before(constraints.axis, self.padding)
            - cross_after(constraints.axis, self.padding))
        .max(0.);
        let before_paint_extent = paint_offset(0., before);
        let before_cache_extent = cache_offset(0., before);
        let inner_scroll = (constraints.scroll_offset - before).max(0.);
        let inner_constraints = SliverConstraints::new(
            constraints.axis,
            constraints.reverse,
            inner_scroll,
            constraints.preceding_scroll_extent + before,
            if constraints.overlap > 0. {
                (constraints.overlap - before_paint_extent).max(0.)
            } else {
                constraints.overlap
            },
            (constraints.remaining_paint_extent - before_paint_extent).max(0.),
            reduced_cross,
            constraints.viewport_main_axis_extent,
            (constraints.remaining_cache_extent - before_cache_extent).max(0.),
            (constraints.cache_origin + before).min(0.),
        );
        let mut layout = self.inner.borrow_mut().perform_layout(inner_constraints);
        let inner_geometry = layout.geometry.normalized();
        if let Some(correction) = inner_geometry.scroll_offset_correction {
            let mut geometry = SliverGeometry::ZERO;
            geometry.scroll_offset_correction = Some(correction);
            return SliverLayout::empty(geometry);
        }
        for child in &mut layout.children {
            child.offset += before;
            child.cross_offset += cross_before(constraints.axis, self.padding);
        }
        let scroll_extent = total_padding + inner_geometry.scroll_extent;
        let after_paint_extent = paint_offset(
            before + inner_geometry.scroll_extent,
            total_padding + inner_geometry.scroll_extent,
        );
        let after_cache_extent = cache_offset(
            before + inner_geometry.scroll_extent,
            total_padding + inner_geometry.scroll_extent,
        );
        let paint_extent = (before_paint_extent
            + inner_geometry
                .paint_extent
                .max(inner_geometry.layout_extent + after_paint_extent))
        .min(constraints.remaining_paint_extent)
        .max(0.);
        let layout_extent =
            (before_paint_extent + after_paint_extent + inner_geometry.layout_extent)
                .min(paint_extent)
                .max(0.);
        layout.geometry = SliverGeometry {
            paint_origin: inner_geometry.paint_origin,
            scroll_extent,
            paint_extent,
            layout_extent,
            cache_extent: (before_cache_extent + after_cache_extent + inner_geometry.cache_extent)
                .min(constraints.remaining_cache_extent)
                .max(0.),
            max_paint_extent: total_padding + inner_geometry.max_paint_extent,
            hit_test_extent: (before_paint_extent
                + after_paint_extent
                + inner_geometry.paint_extent)
                .max(before_paint_extent + inner_geometry.hit_test_extent),
            visible: paint_extent > 0.,
            has_visual_overflow: inner_geometry.has_visual_overflow,
            scroll_offset_correction: None,
        };
        layout
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        self.inner.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.inner.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.inner.borrow().is_animating()
    }
}

pub(super) struct WidgetWrapRenderSliver {
    pub(super) inner: RefCell<Box<dyn RenderSliver>>,
    pub(super) wrap: Rc<dyn Fn(Widget) -> Widget>,
}

pub(super) struct LayoutBuilderRenderSliver {
    pub(super) builder: SliverLayoutBuilderFn,
    pub(super) child: Widget,
    pub(super) extent: Cell<f32>,
    pub(super) last_constraints: Option<SliverConstraints>,
    pub(super) revision: u64,
}

impl RenderSliver for LayoutBuilderRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        if self.last_constraints != Some(constraints) {
            self.child = (self.builder)(constraints);
            self.last_constraints = Some(constraints);
            self.revision = self.revision.wrapping_add(1);
        }
        let extent = self.extent.get().max(0.);
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, extent),
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset: 0.,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    None,
                ),
                extent,
                pinned: false,
            }],
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let changed = (self.extent.get() - extent).abs() > f32::EPSILON;
        self.extent.set(extent.max(0.));
        changed
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}

/// Shared overlap state between an outer absorber and an inner injector.
#[derive(Clone, Default)]
pub struct SliverOverlapHandle {
    extent: Rc<Cell<f32>>,
}

impl SliverOverlapHandle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn extent(&self) -> f32 {
        self.extent.get()
    }

    pub(super) fn set_extent(&self, extent: f32) {
        self.extent.set(extent.max(0.));
    }
}

pub(super) struct OverlapAbsorberRenderSliver {
    pub(super) inner: RefCell<Box<dyn RenderSliver>>,
    pub(super) handle: SliverOverlapHandle,
}

impl RenderSliver for OverlapAbsorberRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let layout = self.inner.borrow_mut().perform_layout(constraints);
        self.handle.set_extent(layout.absorbed_overlap);
        layout
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        self.inner.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.inner.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.inner.borrow().is_animating()
    }
}

pub(super) struct OverlapInjectorRenderSliver {
    pub(super) handle: SliverOverlapHandle,
}

impl RenderSliver for OverlapInjectorRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.handle.extent();
        SliverLayout::empty(SliverGeometry::from_scroll_extent(constraints, extent))
    }
}

impl RenderSliver for WidgetWrapRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let mut layout = self.inner.borrow_mut().perform_layout(constraints);
        for child in &mut layout.children {
            child.widget = (self.wrap)(child.widget.clone());
        }
        layout
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        self.inner.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.inner.borrow().revision()
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.inner.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.inner.borrow().is_animating()
    }
}

/// Sequential viewport implementation shared by `CustomScrollView` and
/// sliver groups. It computes the same per-sliver constraint fields used by a
/// real viewport and scopes child identity at each boundary.
pub(super) struct SequenceRenderSliver {
    pub(super) children: Vec<RefCell<Box<dyn RenderSliver>>>,
}

impl SequenceRenderSliver {
    pub(super) fn new(children: Vec<Box<dyn RenderSliver>>) -> Self {
        Self {
            children: children.into_iter().map(RefCell::new).collect(),
        }
    }

    fn layout_sequence(&self, constraints: SliverConstraints) -> SliverLayout {
        // These are the same independent cursors used by Flutter's
        // RenderViewport.layoutChildSequence. `preceding_scroll_extent` is
        // the logical scroll range consumed by previous slivers, while
        // `layout_offset` is the paint/layout cursor. They intentionally
        // diverge when a sliver is pinned, overlaps content, or has already
        // moved past the trailing edge of the viewport.
        let initial_layout_offset = 0.;
        let mut layout_offset = initial_layout_offset;
        let mut remaining_cache_extent = constraints.remaining_cache_extent;
        let mut cache_origin = constraints.cache_origin;
        let mut scroll_offset = constraints.scroll_offset;
        let mut max_paint_offset = layout_offset + constraints.overlap;
        let mut preceding = 0.;
        let mut children = Vec::new();
        let mut total_correction = None;
        for (sliver_index, sliver) in self.children.iter().enumerate() {
            let sliver_scroll_offset = scroll_offset.max(0.);
            // A sliver must not be asked to cache content before its local
            // scroll offset. This is the same corrected cache-origin rule
            // used by Flutter's viewport and is important when a viewport is
            // partially scrolled into a preceding sliver.
            let corrected_cache_origin = cache_origin.max(-sliver_scroll_offset);
            let cache_extent_correction = cache_origin - corrected_cache_origin;
            let sliver_constraints = SliverConstraints::new(
                constraints.axis,
                constraints.reverse,
                sliver_scroll_offset,
                preceding,
                max_paint_offset - layout_offset,
                (constraints.remaining_paint_extent - layout_offset + initial_layout_offset)
                    .max(0.),
                constraints.cross_axis_extent,
                constraints.viewport_main_axis_extent,
                (remaining_cache_extent + cache_extent_correction).max(0.),
                corrected_cache_origin,
            );
            let layout = sliver.borrow_mut().perform_layout(sliver_constraints);
            let geometry = layout.geometry.normalized();
            if total_correction.is_none() {
                total_correction = geometry.scroll_offset_correction;
            }

            // Flutter restarts the sequence at the first correction. Keep the
            // already laid out prefix so the retained tree remains coherent;
            // the viewport will apply the correction and run this sequence
            // again before painting.
            if geometry.scroll_offset_correction.is_some() {
                break;
            }

            let effective_layout_offset = layout_offset + geometry.paint_origin;
            // Once a sliver is past the trailing edge its effective paint
            // offset is no longer meaningful. Its increasing scroll cursor is
            // still useful for retaining a stable content ordering, matching
            // RenderViewport's fallback placement for invisible slivers.
            let sliver_paint_offset = if geometry.visible || scroll_offset > 0. {
                effective_layout_offset
            } else {
                -scroll_offset + initial_layout_offset
            };
            for mut child in layout.children {
                child.id = SliverChildId::scoped(sliver_index, child.id);
                // The retained tree applies one viewport-level transform.
                // Convert Flutter's paint-space child position back into the
                // sequence's content space so that transform produces the
                // same result for normal, overlapping, and pinned slivers.
                child.offset = sliver_paint_offset + child.offset - sliver_scroll_offset
                    + constraints.scroll_offset;
                children.push(child);
            }

            max_paint_offset =
                max_paint_offset.max(effective_layout_offset + geometry.paint_extent);
            preceding += geometry.scroll_extent;
            scroll_offset -= geometry.scroll_extent;
            layout_offset += geometry.layout_extent;
            if geometry.cache_extent != 0. {
                remaining_cache_extent -= geometry.cache_extent - cache_extent_correction;
                cache_origin = (corrected_cache_origin + geometry.cache_extent).min(0.);
            }
            // Custom slivers may expose an overlap that is not represented by
            // their paint extent. Preserve it as an explicit obstruction;
            // built-in pinned headers report exactly paintExtent-layoutExtent
            // here, so this does not double-count them.
            max_paint_offset =
                max_paint_offset.max(layout_offset + layout.absorbed_overlap.max(0.));
        }
        apply_pinned_offsets_with_direction(
            &mut children,
            constraints.scroll_offset,
            constraints.viewport_main_axis_extent,
            constraints.reverse,
        );
        let mut geometry = SliverGeometry::from_scroll_extent(constraints, preceding);
        geometry.scroll_offset_correction = total_correction;
        SliverLayout {
            geometry,
            children,
            absorbed_overlap: (max_paint_offset - layout_offset).max(0.),
        }
    }
}

impl RenderSliver for SequenceRenderSliver {
    fn child_count(&self) -> Option<usize> {
        self.children.iter().try_fold(0usize, |count, child| {
            child
                .borrow()
                .child_count()
                .map(|child_count| count + child_count)
        })
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.layout_sequence(constraints)
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let index = child.scope();
        self.children
            .get(index)
            .is_some_and(|sliver| sliver.borrow_mut().set_child_extent(child.local(), extent))
    }

    fn revision(&self) -> u64 {
        self.children
            .iter()
            .map(|child| child.borrow().revision())
            .fold(0, u64::wrapping_add)
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.children
            .iter()
            .any(|child| child.borrow_mut().tick(now))
    }

    fn is_animating(&self) -> bool {
        self.children
            .iter()
            .any(|child| child.borrow().is_animating())
    }
}

pub(super) struct SequenceViewportDelegate {
    pub(super) sequence: RefCell<SequenceRenderSliver>,
}

impl SequenceViewportDelegate {
    pub(super) fn new(slivers: Vec<Box<dyn RenderSliver>>) -> Self {
        Self {
            sequence: RefCell::new(SequenceRenderSliver::new(slivers)),
        }
    }
}

impl SliverViewportDelegate for SequenceViewportDelegate {
    fn perform_layout(&self, constraints: SliverConstraints) -> SliverViewportLayout {
        let layout = self.sequence.borrow_mut().perform_layout(constraints);
        SliverViewportLayout {
            geometry: layout.geometry,
            children: layout.children,
        }
    }

    fn set_child_extent(&self, child: SliverChildId, extent: f32) -> bool {
        self.sequence.borrow_mut().set_child_extent(child, extent)
    }

    fn revision(&self) -> u64 {
        self.sequence.borrow().revision()
    }

    fn sliver_count(&self) -> usize {
        self.sequence.borrow().children.len()
    }

    fn child_count(&self) -> Option<usize> {
        self.sequence.borrow().child_count()
    }

    fn tick(&self, now: Instant) -> bool {
        self.sequence.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        self.sequence.borrow().is_animating()
    }
}

/// Builds a viewport for one sliver. Box scrollables such as `ListView` and
/// `PageView` use this same retained sliver protocol as `CustomScrollView`
/// instead of maintaining a second lazy-list implementation.
pub(super) struct SliverViewportOptions {
    pub(super) controller: ScrollController,
    pub(super) axis: Axis,
    pub(super) reverse: bool,
    pub(super) physics: ScrollPhysics,
    pub(super) cache_extent: f32,
    pub(super) clip_behavior: Clip,
    pub(super) shrink_wrap: bool,
}

pub(super) fn single_sliver_viewport(
    controller: ScrollController,
    axis: Axis,
    reverse: bool,
    physics: ScrollPhysics,
    cache_extent: f32,
    clip_behavior: Clip,
    sliver: Box<dyn RenderSliver>,
) -> Widget {
    single_sliver_viewport_with_options(
        SliverViewportOptions {
            controller,
            axis,
            reverse,
            physics,
            cache_extent,
            clip_behavior,
            shrink_wrap: false,
        },
        sliver,
    )
}

pub(super) fn single_sliver_viewport_with_options(
    options: SliverViewportOptions,
    sliver: Box<dyn RenderSliver>,
) -> Widget {
    Widget::sliver_viewport_with_delegate_options(
        options.controller,
        options.axis,
        options.reverse,
        options.physics,
        options.cache_extent,
        options.shrink_wrap,
        options.clip_behavior,
        Rc::new(SequenceViewportDelegate::new(vec![sliver])),
    )
}

pub(super) fn sliver_child_constraints(axis: Axis, cross: f32, extent: Option<f32>) -> Constraints {
    let cross = cross.max(0.);
    match (axis, extent) {
        (Axis::Vertical, Some(extent)) => Constraints::new(cross, cross, extent, extent),
        (Axis::Horizontal, Some(extent)) => Constraints::new(extent, extent, cross, cross),
        (Axis::Vertical, None) => Constraints::new(cross, cross, 0., f32::INFINITY),
        (Axis::Horizontal, None) => Constraints::new(0., f32::INFINITY, cross, cross),
    }
}

/// Returns a conservative main-axis size for a widget that is used as a
/// sliver prototype. This is deliberately limited to dimensions that are
/// independent of the eventual viewport; widgets whose size depends on
/// ambient constraints return `None` and are measured by the retained child
/// pass instead.
pub(super) fn widget_main_extent_hint(widget: &Widget, axis: Axis) -> Option<f32> {
    fn finite(value: f32) -> Option<f32> {
        value.is_finite().then_some(value.max(0.))
    }

    fn dimension(size: Size, axis: Axis) -> Option<f32> {
        finite(axis.main_extent(size))
    }

    fn constrained(child: &Widget, axis: Axis, min: f32, max: f32) -> Option<f32> {
        let min = finite(min).unwrap_or(0.);
        let child = widget_main_extent_hint(child, axis);
        if max.is_finite() {
            let max = max.max(min);
            child
                .map(|value| value.clamp(min, max))
                .or_else(|| finite(max))
        } else {
            child.or_else(|| (min > 0.).then_some(min))
        }
    }

    let result = match &widget.kind {
        WidgetKind::Box { size, .. } => dimension(*size, axis),
        WidgetKind::Shape { size, path, .. } => {
            size.and_then(|size| dimension(size, axis)).or_else(|| {
                path.bounds()
                    .and_then(|bounds| dimension(bounds.size, axis))
            })
        }
        WidgetKind::CustomPaint { size, .. } => dimension(*size, axis),
        WidgetKind::Decorated { size, child, .. } => size
            .and_then(|size| dimension(size, axis))
            .or_else(|| widget_main_extent_hint(child, axis)),
        WidgetKind::Banner { child, .. } => child
            .as_deref()
            .and_then(|child| widget_main_extent_hint(child, axis)),
        WidgetKind::Button { size, child, .. } => dimension(*size, axis)
            .filter(|extent| *extent > 0.)
            .or_else(|| {
                child
                    .as_deref()
                    .and_then(|child| widget_main_extent_hint(child, axis))
            })
            .or_else(|| dimension(*size, axis)),
        WidgetKind::Text { style, .. } | WidgetKind::SelectableText { style, .. } => {
            let font_size = style.size.max(1.);
            let line_height = style
                .line_height
                .map_or(font_size * 1.2, |height| match height {
                    incular_text::LineHeight::Normal => font_size * 1.2,
                    incular_text::LineHeight::Multiplier(multiplier) => {
                        font_size * multiplier.max(0.)
                    }
                    incular_text::LineHeight::Absolute(pixels) => pixels.max(0.),
                });
            finite(line_height)
        }
        WidgetKind::Image { width, height, .. } => finite(match axis {
            Axis::Horizontal => width.unwrap_or(0.),
            Axis::Vertical => height.unwrap_or(0.),
        })
        .filter(|extent| *extent > 0.),
        WidgetKind::TextField { size, .. } => dimension(*size, axis),
        WidgetKind::Padding { padding, child } => {
            widget_main_extent_hint(child, axis).map(|extent| {
                extent
                    + if axis.is_vertical() {
                        padding.top + padding.bottom
                    } else {
                        padding.left + padding.right
                    }
            })
        }
        WidgetKind::Constrained { constraints, child } => constrained(
            child,
            axis,
            if axis.is_vertical() {
                constraints.min_height
            } else {
                constraints.min_width
            },
            if axis.is_vertical() {
                constraints.max_height
            } else {
                constraints.max_width
            },
        ),
        WidgetKind::Limited {
            max_width,
            max_height,
            child,
        } => {
            let max = if axis.is_vertical() {
                *max_height
            } else {
                *max_width
            };
            widget_main_extent_hint(child, axis)
                .map(|extent| extent.min(max))
                .or_else(|| finite(max))
        }
        WidgetKind::Overflow {
            min_width,
            max_width,
            min_height,
            max_height,
            child,
        } => {
            let min = if axis.is_vertical() {
                min_height.unwrap_or(0.)
            } else {
                min_width.unwrap_or(0.)
            };
            let max = if axis.is_vertical() {
                max_height.unwrap_or(f32::INFINITY)
            } else {
                max_width.unwrap_or(f32::INFINITY)
            };
            constrained(child, axis, min, max)
        }
        WidgetKind::Positioned {
            width,
            height,
            child,
            ..
        } => {
            let explicit = if axis.is_vertical() { *height } else { *width };
            explicit
                .and_then(finite)
                .or_else(|| widget_main_extent_hint(child, axis))
        }
        WidgetKind::Visibility { visible, child } => {
            if *visible {
                widget_main_extent_hint(child, axis)
            } else {
                Some(0.)
            }
        }
        WidgetKind::Align { child, .. }
        | WidgetKind::SafeArea { child, .. }
        | WidgetKind::ClipRect { child, .. }
        | WidgetKind::ClipRRect { child, .. }
        | WidgetKind::ClipOval { child, .. }
        | WidgetKind::ClipPath { child, .. }
        | WidgetKind::Gesture { child, .. }
        | WidgetKind::Draggable { child, .. }
        | WidgetKind::DragTarget { child, .. }
        | WidgetKind::IgnorePointer { child, .. }
        | WidgetKind::AbsorbPointer { child, .. }
        | WidgetKind::Unconstrained { child, .. }
        | WidgetKind::RepaintBoundary { child }
        | WidgetKind::FittedBox { child, .. }
        | WidgetKind::Opacity { child, .. }
        | WidgetKind::Blur { child, .. }
        | WidgetKind::DropShadow { child, .. }
        | WidgetKind::ColorFiltered { child, .. }
        | WidgetKind::Blend { child, .. }
        | WidgetKind::Translate { child, .. }
        | WidgetKind::Transform { child, .. }
        | WidgetKind::Scale { child, .. }
        | WidgetKind::Rotation { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::ShaderMask { child, .. }
        | WidgetKind::BackdropFilter { child, .. }
        | WidgetKind::AnnotatedRegion { child, .. }
        | WidgetKind::CompositedTransformTarget { child, .. }
        | WidgetKind::CompositedTransformFollower { child, .. } => {
            widget_main_extent_hint(child, axis)
        }
        WidgetKind::RawInput { child, .. } => child
            .as_deref()
            .and_then(|child| widget_main_extent_hint(child, axis)),
        WidgetKind::Baseline { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::Flexible { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::Flex {
            axis: flex_axis,
            children,
            spacing,
            ..
        } => {
            let hints = children
                .iter()
                .map(|child| widget_main_extent_hint(child, axis))
                .collect::<Option<Vec<_>>>()?;
            if *flex_axis == axis {
                let spacing = spacing.max(0.) * children.len().saturating_sub(1) as f32;
                finite(hints.into_iter().sum::<f32>() + spacing)
            } else {
                hints.into_iter().reduce(f32::max).or(Some(0.))
            }
        }
        WidgetKind::Stack { children, .. } | WidgetKind::IndexedStack { children, .. } => children
            .iter()
            .filter_map(|child| widget_main_extent_hint(child, axis))
            .reduce(f32::max),
        WidgetKind::SelectionArea { child, .. }
        | WidgetKind::SelectionContainer { child, .. }
        | WidgetKind::SelectionListener { child, .. }
        | WidgetKind::IndexedSemantics { child, .. }
        | WidgetKind::SemanticsDebugger { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::PersistentHeader { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::NotificationListener { child, .. } => widget_main_extent_hint(child, axis),
        WidgetKind::RawScrollbar { child, .. }
        | WidgetKind::DraggableScrollableActuator { child, .. } => {
            widget_main_extent_hint(child, axis)
        }
        // A scrollable or a layout builder obtains its main-axis extent from
        // its parent; guessing it from the child would make a prototype list
        // report a different extent from the actual viewport.
        WidgetKind::Scroll { .. }
        | WidgetKind::ListWheelScrollView { .. }
        | WidgetKind::ListWheelViewport { .. }
        | WidgetKind::DraggableScrollableSheet { .. }
        | WidgetKind::TwoDimensionalScrollView { .. }
        | WidgetKind::TwoDimensionalViewport { .. }
        | WidgetKind::SliverViewport { .. }
        | WidgetKind::LayoutBuilder { .. }
        | WidgetKind::AspectRatio { .. }
        | WidgetKind::Fractional { .. }
        | WidgetKind::Wrap { .. }
        | WidgetKind::Table { .. } => None,
    };
    result.and_then(finite)
}

fn main_before(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.top
    } else {
        padding.left
    }
}

fn main_after(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.bottom
    } else {
        padding.right
    }
}

fn cross_before(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.left
    } else {
        padding.top
    }
}

fn cross_after(axis: Axis, padding: EdgeInsets) -> f32 {
    if axis.is_vertical() {
        padding.right
    } else {
        padding.bottom
    }
}

fn apply_pinned_offsets_with_direction(
    children: &mut [SliverChildLayout],
    scroll: f32,
    viewport: f32,
    reverse: bool,
) {
    let viewport = viewport.max(0.);
    let pinned = children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| child.pinned.then_some(index))
        .collect::<Vec<_>>();
    if reverse {
        // In a reversed viewport the leading edge is the physical trailing
        // edge. Walk backwards so multiple pinned headers stack from right to
        // left (or bottom to top) in the same way Flutter's viewport does.
        let mut stack = viewport;
        for child_index in pinned.into_iter().rev() {
            let normal = children[child_index].offset;
            let extent = children[child_index].extent.max(0.).min(viewport);
            let current = normal - scroll;
            let target = stack - extent;
            if current <= target {
                children[child_index].offset = normal + target - current;
                stack = target;
            }
        }
    } else {
        // Pinned headers reserve a slot at the leading edge once their normal
        // position reaches that slot. Every subsequent pinned header uses the
        // end of the previous slot, preventing overlap while preserving the
        // normal flow position before it reaches the stack.
        let mut stack = 0.;
        for child_index in pinned {
            let normal = children[child_index].offset;
            let extent = children[child_index].extent.max(0.).min(viewport);
            let current = normal - scroll;
            if current <= stack {
                children[child_index].offset = normal + stack - current;
                stack += extent;
            }
        }
    }
}
