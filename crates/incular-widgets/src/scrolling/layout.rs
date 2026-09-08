use super::*;
use std::any::Any;

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
                placement: if self.pinned {
                    SliverChildPlacement::Pinned
                } else {
                    SliverChildPlacement::Flow
                },
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

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
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
                    placement: SliverChildPlacement::Flow,
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
                    placement: SliverChildPlacement::Flow,
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
                    placement: SliverChildPlacement::Flow,
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
/// viewport starts moving back toward the leading edge. When `snap` is set,
/// scroll-activity ends animate the effective offset to the revealed or
/// hidden edge instead of leaving partial presentation in place.
pub(super) struct FloatingHeaderRenderSliver {
    pub(super) child: Widget,
    pub(super) extent: Cell<f32>,
    pub(super) scroll_state: HeaderScrollState,
    pub(super) snap: bool,
    pub(super) snap_frame: Cell<HeaderSnapFrame>,
    pub(super) snap_activity: Rc<Cell<HeaderSnapActivity>>,
    /// Owns the scroll-activity listener while snapping can engage. Never read:
    /// dropping the subscription unsubscribes, so disposal stops snap
    /// triggers without further cleanup. `None` when snapping cannot engage.
    #[allow(dead_code)]
    pub(super) snap_subscription: Option<ScrollNotificationSubscription>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct HeaderScrollState {
    last_scroll_offset: Option<f32>,
    effective_scroll_offset: f32,
    snap: HeaderSnapProgress,
}

impl HeaderScrollState {
    fn update(&mut self, scroll_offset: f32, extent: f32) -> f32 {
        if let Some(previous) = self.last_scroll_offset {
            if scroll_offset != previous {
                // New scroll movement interrupts a running snap; the
                // animation's current presentation becomes the baseline the
                // delta continues from, so interruption never jumps.
                self.snap = HeaderSnapProgress::Idle;
            }
            // Hidden distance stops at the header extent. Accumulating the
            // rest of the document would delay revealing it on reversal.
            self.effective_scroll_offset = (self.effective_scroll_offset
                + (scroll_offset - previous))
                .clamp(0., extent)
                .min(scroll_offset);
        } else {
            self.effective_scroll_offset = scroll_offset.min(extent);
        }
        self.last_scroll_offset = Some(scroll_offset);
        self.effective_scroll_offset
    }

    /// Drops a running snap without touching the effective offset, so
    /// presentation falls back to whatever the scroll offset determines.
    pub(super) fn cancel_snap(&mut self) {
        self.snap = HeaderSnapProgress::Idle;
    }

    /// Whether snap work remains: a running animation, or a completed one
    /// whose exact endpoint still needs one presenting frame.
    pub(super) fn is_snapping(&self) -> bool {
        matches!(
            self.snap,
            HeaderSnapProgress::Running(_) | HeaderSnapProgress::Settling
        )
    }

    /// Consumes pending scroll activity and advances snap presentation.
    ///
    /// `spec` carries the header's snap configuration and live snap range;
    /// `frame` is the last layout's paint/overlap/scroll snapshot. A new
    /// activity cancels running and settling work first — even when the
    /// offset has not moved — while preserving the current presentation as
    /// the baseline later movement continues from. A run starts only from an
    /// actual scroll-end signal while the header is partially revealed and
    /// free of leading overscroll — never from an unchanged offset. The
    /// revealed half (or more) animates to fully revealed; the hidden half
    /// animates to fully hidden, clamped to the scrolled distance the scroll
    /// state itself enforces. The logical scroll extent and controller offset
    /// are untouched: only the effective (presentation) offset moves, and on
    /// completion it equals the endpoint exactly so later scrolls continue
    /// coherently. Returns whether the effective offset moved.
    pub(super) fn advance_snap(
        &mut self,
        now: Instant,
        activity: HeaderSnapActivity,
        spec: HeaderSnapSpec,
        frame: HeaderSnapFrame,
    ) -> bool {
        if activity == HeaderSnapActivity::Started {
            self.snap = HeaderSnapProgress::Idle;
            return false;
        }
        let range = spec.range.max(0.);
        if activity == HeaderSnapActivity::Ended
            && spec.enabled
            && spec.floating
            && range > 0.
            && frame.overlap >= 0.
            && frame.scroll.is_finite()
        {
            let visible = (frame.paint - spec.pinned).clamp(0., range);
            // The hidden edge can never lie past the scrolled distance:
            // scroll-driven presentation clamps hidden distance to it, so a
            // target past it would fight every layout back. When the header
            // is already maximally hidden for its offset, the target equals
            // the current presentation and no run starts.
            let target = if visible >= range / 2. {
                0.
            } else {
                range.min(frame.scroll).max(0.)
            };
            let from = self.effective_scroll_offset.clamp(0., range);
            self.snap = if target == from {
                HeaderSnapProgress::Idle
            } else {
                HeaderSnapProgress::Running(HeaderSnapRun {
                    from,
                    target,
                    started_at: now,
                    duration: HEADER_SNAP_DURATION,
                    curve: Curve::EaseOut,
                })
            };
        }
        let HeaderSnapProgress::Running(run) = self.snap else {
            if matches!(self.snap, HeaderSnapProgress::Settling) {
                self.snap = HeaderSnapProgress::Idle;
            }
            return false;
        };
        let target = run.target.clamp(0., range);
        let elapsed = now.saturating_duration_since(run.started_at);
        let progress = (elapsed.as_secs_f32() / run.duration.as_secs_f32()).clamp(0., 1.);
        let value = if progress >= 1. {
            target
        } else {
            f32::interpolate(run.from, target, run.curve.apply(progress))
        };
        let changed = value != self.effective_scroll_offset;
        self.effective_scroll_offset = value;
        if progress >= 1. {
            self.snap = HeaderSnapProgress::Settling;
        }
        changed
    }
}

/// Neutral snap timing: one fixed duration and curve shared by every floating
/// header, so snapping stays a presentation policy rather than per-header
/// configuration.
const HEADER_SNAP_DURATION: Duration = Duration::from_millis(300);

/// Unconsumed scroll-activity signal for snap decisions. Each event
/// overwrites the previous one, which preserves ordering across frames: End
/// followed by Start leaves `Started` (no snap may begin inside the new
/// activity), while Start followed by End leaves `Ended` (a fresh endpoint
/// decision runs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HeaderSnapActivity {
    /// No unconsumed activity transition.
    #[default]
    None,
    /// A scroll activity began: cancel snap work, start nothing.
    Started,
    /// A scroll activity ended: a partially revealed header may snap.
    Ended,
}

/// Mutually exclusive snap presentation states. A running animation carries
/// its start presentation, resolved target, and timing together; there are no
/// partially valid flag combinations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum HeaderSnapProgress {
    /// No snap animation; presentation follows scroll deltas.
    #[default]
    Idle,
    /// A snap animation is driving the effective offset.
    Running(HeaderSnapRun),
    /// The animation reached its endpoint exactly on the last tick. One more
    /// frame stays scheduled so the exact endpoint is laid out and presented
    /// before the state returns to idle; without it the completing tick would
    /// mark layout dirty while reporting no active animation, and no frame
    /// would present the endpoint.
    Settling,
}

/// One snap animation: where presentation started, the resolved absolute
/// effective target, and when/how fast it gets there. The target is resolved
/// against the live range every tick, so geometry changes retarget the run
/// instead of stranding it past a new edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HeaderSnapRun {
    from: f32,
    target: f32,
    started_at: Instant,
    duration: Duration,
    curve: Curve,
}

/// A header's snap configuration plus its live snap range for one tick.
#[derive(Clone, Copy, Debug)]
pub(super) struct HeaderSnapSpec {
    /// The descriptor enabled snapping.
    pub(super) enabled: bool,
    /// The behavior presents a floating range (Floating or FloatingPinned).
    pub(super) floating: bool,
    /// Snap travel: the full extent for Floating, the collapse range for
    /// FloatingPinned.
    pub(super) range: f32,
    /// The minimum that stays visible and is excluded from snap travel (the
    /// pinned minimum for FloatingPinned, zero for Floating).
    pub(super) pinned: f32,
}

/// The last layout's paint/overlap snapshot a snap tick decides from.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct HeaderSnapFrame {
    /// Painted extent of the last layout.
    pub(super) paint: f32,
    /// Constraint overlap of the last layout; negative means leading
    /// overscroll is presenting instead.
    pub(super) overlap: f32,
    /// Scroll offset the last layout ran under; hidden targets can never
    /// exceed it, since scroll-driven presentation clamps hidden distance to
    /// the scrolled distance.
    pub(super) scroll: f32,
}

/// Creates the shared scroll-activity signal and its controller subscription
/// for a header that can snap. The listener records actual scroll-activity
/// `Start` and `End` transitions, each overwriting the previous one, so an
/// `End` stranded inside a newer activity can never start a snap; starting
/// from unchanged offsets is never inferred. Pass `false` unless snapping
/// can engage, so inert headers keep their existing behavior exactly. The
/// returned subscription must be retained by the render sliver: dropping it
/// unsubscribes, so disposal stops snap signals without further cleanup.
pub(super) fn snap_activity_subscription(
    controller: &ScrollController,
    enabled: bool,
) -> (
    Rc<Cell<HeaderSnapActivity>>,
    Option<ScrollNotificationSubscription>,
) {
    let activity = Rc::new(Cell::new(HeaderSnapActivity::None));
    let subscription = enabled.then(|| {
        let activity = activity.clone();
        controller.add_notification_listener(move |notification| {
            match notification.kind {
                ScrollNotificationType::Start => activity.set(HeaderSnapActivity::Started),
                ScrollNotificationType::End => activity.set(HeaderSnapActivity::Ended),
                _ => {}
            }
            false
        })
    });
    (activity, subscription)
}

fn floating_geometry(
    constraints: SliverConstraints,
    extent: f32,
    effective_offset: f32,
) -> SliverGeometry {
    let scroll_offset = constraints.scroll_offset;
    let effective_remaining_paint_extent =
        (constraints.remaining_paint_extent - constraints.overlap).max(0.);
    let paint_extent = (extent - effective_offset)
        .max(0.)
        .min(effective_remaining_paint_extent);
    let layout_extent = (extent - scroll_offset)
        .clamp(0., effective_remaining_paint_extent)
        .min(paint_extent);
    SliverGeometry {
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
    }
}

impl RenderSliver for FloatingHeaderRenderSliver {
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        SliverScrollDependency::ScrollOffset
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let extent = self.extent.get().max(0.);
        let scroll_offset = constraints.scroll_offset;
        let effective_offset = self.scroll_state.update(scroll_offset, extent);
        let geometry = floating_geometry(constraints, extent, effective_offset);
        self.snap_frame.set(HeaderSnapFrame {
            paint: geometry.paint_extent,
            overlap: constraints.overlap,
            scroll: constraints.scroll_offset,
        });
        SliverLayout {
            geometry,
            // The sequence converts this back through the viewport transform.
            // `extent - effective` is not a normal flow offset: it exposes
            // the child at the leading edge while the header is floating.
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset: scroll_offset - effective_offset,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    None,
                ),
                extent,
                placement: SliverChildPlacement::Floating,
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

    fn tick(&mut self, now: Instant) -> bool {
        // Consuming here (before advancing) applies the ordering rule: a
        // newer Start already overwrote any stranded End.
        let activity = self.snap_activity.take();
        self.scroll_state.advance_snap(
            now,
            activity,
            HeaderSnapSpec {
                enabled: self.snap,
                floating: true,
                range: self.extent.get().max(0.),
                pinned: 0.,
            },
            self.snap_frame.get(),
        )
    }

    fn is_animating(&self) -> bool {
        self.scroll_state.is_snapping()
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
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
                placement: if self.pinned {
                    SliverChildPlacement::Pinned
                } else {
                    SliverChildPlacement::Flow
                },
            }],
            absorbed_overlap: (geometry.paint_extent - geometry.layout_extent).max(0.),
        }
    }
}

pub(super) struct ResizingHeaderRenderSliver {
    pub(super) child: Widget,
    pub(super) min_extent: f32,
    pub(super) max_extent: f32,
    pub(super) scroll_behavior: SliverHeaderScrollBehavior,
    pub(super) overscroll_behavior: SliverHeaderOverscrollBehavior,
    pub(super) scroll_state: HeaderScrollState,
    pub(super) snap: bool,
    pub(super) snap_frame: Cell<HeaderSnapFrame>,
    pub(super) snap_activity: Rc<Cell<HeaderSnapActivity>>,
    /// Owns the scroll-activity listener while snapping can engage. Never read:
    /// dropping the subscription unsubscribes, so disposal stops snap
    /// triggers without further cleanup. `None` when snapping cannot engage.
    #[allow(dead_code)]
    pub(super) snap_subscription: Option<ScrollNotificationSubscription>,
}

/// Lifecycle of a naturally measured header's logical extent.
///
/// Validity depends on the actual measurement inputs, not on position or
/// type correspondence:
///
/// - An unverified estimate seeds the first frame so a new header has
///   geometry before its child is measured. It never authorizes stretched
///   presentation: estimates always measure unbounded first, so a header
///   created during overscroll establishes its true natural size instead of
///   mistaking stretched visuals (or a blind default) for logical extent.
/// - A validated measurement is tied to the cross-axis extent it was recorded
///   under (wrapping content re-flows when the viewport width changes). A
///   cross change demotes it back to an estimate for unbounded revalidation,
///   including mid-overscroll; any genuine range change then settles through
///   Scroll's documented extent policy.
/// - Transferred measurements arrive as estimate seeds, never as validity:
///   matching positions and types prove nothing about current content, so
///   every seed revalidates unbounded before stretched presentation applies.
///   Seeding with the predecessor's validated value (rather than a blind
///   hint) keeps equivalent replacements range-stable through validation,
///   while changed content authoritatively re-establishes itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum NaturalHeaderExtent {
    /// Unverified first estimate: a transferred predecessor value or a child
    /// hint / lazy default. Always measures unbounded next.
    Estimate(f32),
    /// Validated by retained child measurement under the recorded cross-axis
    /// extent.
    Measured { natural: f32, cross: f32 },
}

/// Whether the most recent layout presented stretched visuals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HeaderPresentation {
    /// Normal presentation; child measurements describe logical extent.
    #[default]
    Settled,
    /// Leading overscroll presentation; child measurements are transient.
    Stretched,
}

/// Retained naturally measured header with optional leading stretch.
///
/// Only unbounded child measurements may establish or update the measured
/// extent. Stretched (tight) samples are presentation-only and can never
/// accumulate into the logical scroll range: tight totals conflate natural
/// size with transient overscroll, so they carry no validity signal. A
/// revision-driven descendant rebuild demotes back to an estimate instead,
/// so in-place content changes revalidate unbounded even mid-overscroll;
/// the tight presentation still resolves the live-measured bottom with the
/// toolbar taking the remainder every pass.
pub(super) struct NaturalHeaderRenderSliver {
    pub(super) child: Widget,
    pub(super) scroll_behavior: SliverHeaderScrollBehavior,
    pub(super) overscroll_behavior: SliverHeaderOverscrollBehavior,
    pub(super) extent: Cell<NaturalHeaderExtent>,
    pub(super) presentation: Cell<HeaderPresentation>,
    pub(super) last_cross: Cell<f32>,
    pub(super) sample_unbounded: Cell<bool>,
    pub(super) scroll_state: HeaderScrollState,
    pub(super) snap: bool,
    pub(super) snap_frame: Cell<HeaderSnapFrame>,
    pub(super) snap_activity: Rc<Cell<HeaderSnapActivity>>,
    /// Owns the scroll-activity listener while snapping can engage. Never read:
    /// dropping the subscription unsubscribes, so disposal stops snap
    /// triggers without further cleanup. `None` when snapping cannot engage.
    #[allow(dead_code)]
    pub(super) snap_subscription: Option<ScrollNotificationSubscription>,
}

impl NaturalHeaderRenderSliver {
    /// Compatibility and invalidation rules for transfer across descriptor
    /// updates (applied pairwise by sliver position). Returns whether any
    /// state was adopted.
    ///
    /// - A predecessor's latest number is adopted only as an unverified
    ///   estimate seed, never as validity — even for identical positions and
    ///   types. The seed revalidates unbounded, so equivalent replacements
    ///   stay range-stable while changed content cannot retain a stale
    ///   measurement. Seeding from the predecessor (rather than a blind
    ///   hint) also survives a drain demotion that ran moments earlier on
    ///   the old delegate during the same update.
    /// - Reversal tracking transfers only when `scroll_behavior` is identical,
    ///   since it defines the effective-offset range semantics.
    /// - Stretched presentation is never inherited; it recomputes from live
    ///   overlap on the next layout.
    pub(super) fn adopt_compatible_state(&mut self, previous: &Self) -> bool {
        let mut adopted = false;
        let seed = match previous.extent.get() {
            NaturalHeaderExtent::Estimate(estimate) => estimate,
            NaturalHeaderExtent::Measured { natural, .. } => natural,
        }
        .max(0.);
        // A never-measured predecessor carries no information beyond the
        // fresh hint seed unless invalidation demoted it — but a demoted
        // seed is exactly as good, so adopt uniformly and revalidate.
        if !matches!(self.extent.get(), NaturalHeaderExtent::Estimate(current) if (current - seed).abs() <= f32::EPSILON)
        {
            self.extent.set(NaturalHeaderExtent::Estimate(seed));
            adopted = true;
        }
        if self.scroll_behavior == previous.scroll_behavior {
            self.scroll_state = previous.scroll_state;
            adopted = true;
        }
        // A running snap never transfers: replacement always re-seeds the
        // extent to an unverified estimate, so the snap range basis is
        // uncertain until revalidation. Presentation falls back to whatever
        // the scroll offset determines, which is always coherent.
        self.scroll_state.cancel_snap();
        adopted
    }
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
                placement: SliverChildPlacement::Flow,
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
                    placement: SliverChildPlacement::Flow,
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
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        SliverScrollDependency::ScrollOffset
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let range = self.max_extent - self.min_extent;
        let scroll_offset = constraints.scroll_offset;
        let effective_offset = match self.scroll_behavior {
            SliverHeaderScrollBehavior::Floating => {
                self.scroll_state.update(scroll_offset, self.max_extent)
            }
            SliverHeaderScrollBehavior::FloatingPinned => {
                self.scroll_state.update(scroll_offset, range)
            }
            _ => scroll_offset,
        };
        let stretch = match self.overscroll_behavior {
            SliverHeaderOverscrollBehavior::Stretch
                if scroll_offset == 0. && constraints.preceding_scroll_extent == 0. =>
            {
                -constraints.overlap.min(0.)
            }
            _ => 0.,
        };
        let current = ((self.max_extent - effective_offset)
            .clamp(self.min_extent, self.max_extent)
            + stretch)
            .min(f32::MAX);
        if stretch > 0. {
            // Stretch owns presentation during leading overscroll; a running
            // snap yields to it and the next scroll end decides again.
            self.scroll_state.cancel_snap();
        }
        let (mut geometry, offset, mut placement) = match self.scroll_behavior {
            SliverHeaderScrollBehavior::Pinned | SliverHeaderScrollBehavior::FloatingPinned => (
                pinned_geometry(constraints, self.max_extent, current),
                0.,
                SliverChildPlacement::Pinned,
            ),
            SliverHeaderScrollBehavior::Scroll => (
                SliverGeometry::from_scroll_extent(constraints, self.max_extent),
                scroll_offset.min(range),
                SliverChildPlacement::Flow,
            ),
            SliverHeaderScrollBehavior::Floating => (
                floating_geometry(constraints, self.max_extent, effective_offset),
                scroll_offset - effective_offset + effective_offset.min(range),
                SliverChildPlacement::Floating,
            ),
        };
        if stretch > 0. {
            let available = (constraints.remaining_paint_extent + stretch).min(f32::MAX);
            geometry.paint_extent = current.min(available);
            geometry.hit_test_extent = geometry.paint_extent;
            geometry.max_paint_extent = current;
            geometry.paint_origin = -stretch;
            geometry.visible = geometry.paint_extent > 0.;
            // This placement cancels the viewport's overscroll translation.
            // Automatic pinning would move it back down and leave a gap.
            placement = SliverChildPlacement::Floating;
        }
        self.snap_frame.set(HeaderSnapFrame {
            paint: geometry.paint_extent,
            overlap: constraints.overlap,
            scroll: constraints.scroll_offset,
        });
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset,
                cross_offset: 0.,
                constraints: sliver_child_constraints(
                    constraints.axis,
                    constraints.cross_axis_extent,
                    Some(current),
                ),
                extent: current,
                placement,
            }],
            absorbed_overlap: if stretch > 0. {
                0.
            } else {
                (geometry.paint_extent - geometry.layout_extent).max(0.)
            },
        }
    }

    fn tick(&mut self, now: Instant) -> bool {
        // Consuming here (before advancing) applies the ordering rule: a
        // newer Start already overwrote any stranded End.
        let activity = self.snap_activity.take();
        let (range, pinned) = match self.scroll_behavior {
            SliverHeaderScrollBehavior::Floating => (self.max_extent, 0.),
            SliverHeaderScrollBehavior::FloatingPinned => {
                ((self.max_extent - self.min_extent).max(0.), self.min_extent)
            }
            _ => (0., 0.),
        };
        self.scroll_state.advance_snap(
            now,
            activity,
            HeaderSnapSpec {
                enabled: self.snap,
                floating: matches!(
                    self.scroll_behavior,
                    SliverHeaderScrollBehavior::Floating
                        | SliverHeaderScrollBehavior::FloatingPinned
                ),
                range,
                pinned,
            },
            self.snap_frame.get(),
        )
    }

    fn is_animating(&self) -> bool {
        self.scroll_state.is_snapping()
    }
}

impl RenderSliver for NaturalHeaderRenderSliver {
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        SliverScrollDependency::ScrollOffset
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let cross = constraints.cross_axis_extent.max(0.);
        self.last_cross.set(cross);
        // A validated measurement only covers the cross extent recorded with
        // it: wrapping content re-flows when the viewport width changes, so a
        // cross change demotes back to an estimate for unbounded revalidation
        // — including mid-overscroll, where any genuine range change then
        // settles through Scroll's documented extent policy.
        if let NaturalHeaderExtent::Measured {
            natural,
            cross: validated,
        } = self.extent.get()
            && (validated - cross).abs() > f32::EPSILON
        {
            self.extent
                .set(NaturalHeaderExtent::Estimate(natural.max(0.)));
        }
        let (natural, unverified) = match self.extent.get() {
            NaturalHeaderExtent::Estimate(estimate) => (estimate.max(0.), true),
            NaturalHeaderExtent::Measured { natural, .. } => (natural.max(0.), false),
        };
        let scroll_offset = constraints.scroll_offset;
        let effective_offset = match self.scroll_behavior {
            SliverHeaderScrollBehavior::Floating => {
                self.scroll_state.update(scroll_offset, natural)
            }
            SliverHeaderScrollBehavior::FloatingPinned => {
                // A fixed natural header has no collapse range; keep the
                // collapsed-equivalent extent pinned while still tracking the
                // reversal state for a consistent lifecycle.
                self.scroll_state.update(scroll_offset, 0.)
            }
            _ => scroll_offset,
        };
        let stretch = match self.overscroll_behavior {
            SliverHeaderOverscrollBehavior::Stretch
                if scroll_offset == 0. && constraints.preceding_scroll_extent == 0. =>
            {
                -constraints.overlap.min(0.)
            }
            _ => 0.,
        };
        let stretched_now = stretch > 0.;
        self.presentation.set(if stretched_now {
            HeaderPresentation::Stretched
        } else {
            HeaderPresentation::Settled
        });
        if stretched_now {
            // Stretch owns presentation during leading overscroll; a running
            // snap yields to it and the next scroll end decides again.
            self.scroll_state.cancel_snap();
        }
        let current = (natural + stretch).min(f32::MAX);
        let (mut geometry, offset, mut placement) = match self.scroll_behavior {
            SliverHeaderScrollBehavior::Pinned | SliverHeaderScrollBehavior::FloatingPinned => (
                pinned_geometry(constraints, natural, current),
                0.,
                SliverChildPlacement::Pinned,
            ),
            SliverHeaderScrollBehavior::Scroll => (
                SliverGeometry::from_scroll_extent(constraints, natural),
                0.,
                SliverChildPlacement::Flow,
            ),
            SliverHeaderScrollBehavior::Floating => (
                floating_geometry(constraints, natural, effective_offset),
                scroll_offset - effective_offset,
                SliverChildPlacement::Floating,
            ),
        };
        if stretch > 0. {
            let available = (constraints.remaining_paint_extent + stretch).min(f32::MAX);
            geometry.paint_extent = current.min(available);
            geometry.hit_test_extent = geometry.paint_extent;
            geometry.max_paint_extent = current;
            geometry.paint_origin = -stretch;
            geometry.visible = geometry.paint_extent > 0.;
            // This placement cancels the viewport's overscroll translation.
            // Automatic pinning would move it back down and leave a gap.
            placement = SliverChildPlacement::Floating;
        }
        // Stretched presentation uses tight constraints so a toolbar can fill
        // the extra extent. Settled layouts stay unbounded on the main axis
        // so later content changes are measured instead of being clamped to
        // a previously cached extent. Unverified estimates always measure
        // unbounded first, even while stretched, so a header created during
        // overscroll learns its true size; the estimate only seeds one
        // transient presentation frame, which the viewport's bounded
        // re-layout pass then corrects.
        let unbounded_sample = unverified || !stretched_now;
        self.sample_unbounded.set(unbounded_sample);
        let child_constraints = if unbounded_sample {
            sliver_child_constraints(constraints.axis, constraints.cross_axis_extent, None)
        } else {
            sliver_child_constraints(
                constraints.axis,
                constraints.cross_axis_extent,
                Some(current),
            )
        };
        self.snap_frame.set(HeaderSnapFrame {
            paint: geometry.paint_extent,
            overlap: constraints.overlap,
            scroll: constraints.scroll_offset,
        });
        SliverLayout {
            geometry,
            children: vec![SliverChildLayout {
                id: SliverChildId(0),
                widget: self.child.clone(),
                semantic_index: None,
                offset,
                cross_offset: 0.,
                constraints: child_constraints,
                extent: current,
                placement,
            }],
            absorbed_overlap: if stretch > 0. {
                0.
            } else {
                (geometry.paint_extent - geometry.layout_extent).max(0.)
            },
        }
    }

    fn tick(&mut self, now: Instant) -> bool {
        // Consuming here (before advancing) applies the ordering rule: a
        // newer Start already overwrote any stranded End.
        let activity = self.snap_activity.take();
        let natural = match self.extent.get() {
            NaturalHeaderExtent::Estimate(estimate) => estimate.max(0.),
            NaturalHeaderExtent::Measured { natural, .. } => natural.max(0.),
        };
        // A floating-pinned natural header has no collapse range, so its snap
        // endpoints coincide and the run below stays inert by construction.
        let (range, pinned) = match self.scroll_behavior {
            SliverHeaderScrollBehavior::Floating => (natural, 0.),
            SliverHeaderScrollBehavior::FloatingPinned => (0., natural),
            _ => (0., 0.),
        };
        self.scroll_state.advance_snap(
            now,
            activity,
            HeaderSnapSpec {
                enabled: self.snap,
                floating: matches!(
                    self.scroll_behavior,
                    SliverHeaderScrollBehavior::Floating
                        | SliverHeaderScrollBehavior::FloatingPinned
                ),
                range,
                pinned,
            },
            self.snap_frame.get(),
        )
    }

    fn is_animating(&self) -> bool {
        self.scroll_state.is_snapping()
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if child.0 != 0 || !extent.is_finite() || extent < 0. {
            return false;
        }
        let extent = extent.max(0.);
        match self.extent.get() {
            // Unbounded learning samples are the true natural size by
            // construction. Adopting one can never mistake stretched visuals
            // for logical extent. The transition always requests another
            // layout — even when the value equals the estimate — because
            // presentation constraints must still switch (unbounded learning
            // to settled or tight stretch). A tight sample against an
            // estimate only happens when content was invalidated mid-pass
            // after tight constraints went out; it carries no validity, so
            // the estimate stands while the follow-up pass revalidates.
            NaturalHeaderExtent::Estimate(_) if self.sample_unbounded.get() => {
                self.extent.set(NaturalHeaderExtent::Measured {
                    natural: extent,
                    cross: self.last_cross.get(),
                });
                true
            }
            NaturalHeaderExtent::Estimate(_) => true,
            // Tight samples conflate natural size with transient overscroll
            // and carry no validity signal, so they are ignored: ignoring
            // them keeps overscroll from drifting the range across repeated
            // stretch/recovery cycles.
            NaturalHeaderExtent::Measured { .. }
                if self.presentation.get() == HeaderPresentation::Stretched =>
            {
                false
            }
            NaturalHeaderExtent::Measured { natural, cross } => {
                if (natural - extent).abs() <= f32::EPSILON {
                    return false;
                }
                self.extent.set(NaturalHeaderExtent::Measured {
                    natural: extent,
                    cross,
                });
                true
            }
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn invalidate_child_measurement(&mut self, _child: SliverChildId) {
        // A rebuilt descendant means new content: drop validity back to an
        // estimate seeded with the last validated value. The next layout
        // revalidates unbounded (even mid-overscroll), so equivalent content
        // reconverges without range movement while changed content cannot
        // retain staleness.
        if let NaturalHeaderExtent::Measured { natural, .. } = self.extent.get() {
            self.extent
                .set(NaturalHeaderExtent::Estimate(natural.max(0.)));
        }
    }
}

pub(super) struct PaddingRenderSliver {
    pub(super) inner: RefCell<Box<dyn RenderSliver>>,
    pub(super) padding: EdgeInsets,
}

impl RenderSliver for PaddingRenderSliver {
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        self.inner.borrow().scroll_layout_dependency()
    }

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

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn invalidate_child_measurement(&mut self, child: SliverChildId) {
        self.inner.borrow_mut().invalidate_child_measurement(child);
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
                placement: SliverChildPlacement::Flow,
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
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        self.inner.borrow().scroll_layout_dependency()
    }

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

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn invalidate_child_measurement(&mut self, child: SliverChildId) {
        self.inner.borrow_mut().invalidate_child_measurement(child);
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
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        self.inner.borrow().scroll_layout_dependency()
    }

    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let mut layout = self.inner.borrow_mut().perform_layout(constraints);
        for child in &mut layout.children {
            child.widget = (self.wrap)(child.widget.clone());
        }
        layout
    }

    fn invalidate_child_measurement(&mut self, child: SliverChildId) {
        self.inner.borrow_mut().invalidate_child_measurement(child);
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

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
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
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        self.children
            .iter()
            .map(|child| child.borrow().scroll_layout_dependency())
            .max()
            .unwrap_or(SliverScrollDependency::CacheWindow)
    }

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

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn invalidate_child_measurement(&mut self, child: SliverChildId) {
        // Route exactly like set_child_extent so invalidation reaches the
        // sliver that owns the rebuilt descendant and no other.
        let index = child.scope();
        if let Some(sliver) = self.children.get(index) {
            sliver
                .borrow_mut()
                .invalidate_child_measurement(child.local());
        }
    }
}

/// Transfers validated natural-header measurement from a retained sliver into
/// its freshly built replacement, recursing by position through sequences
/// and transparent single-inner wrappers. Only validated measurements move;
/// stretched presentation, estimates, and incompatible shapes stay fresh, so
/// replacing a viewport descriptor during overscroll keeps the true size
/// without flashing an estimate or spuriously changing the scroll range.
pub(crate) fn transfer_retained_sliver_state(
    fresh: &mut dyn RenderSliver,
    retained: &mut dyn RenderSliver,
) {
    if adopt_natural_header_state(fresh, retained)
        || adopt_box_extent(fresh, retained)
        || adopt_floating_header_state(fresh, retained)
    {
        return;
    }
    if let (Some(fresh), Some(retained)) = (
        fresh
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<SequenceRenderSliver>()),
        retained
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<SequenceRenderSliver>()),
    ) {
        for (fresh_child, retained_child) in fresh.children.iter().zip(retained.children.iter()) {
            let mut fresh = fresh_child.borrow_mut();
            let mut retained = retained_child.borrow_mut();
            transfer_retained_sliver_state(&mut **fresh, &mut **retained);
        }
        return;
    }
    transfer_single_inner_sliver_state::<PaddingRenderSliver>(fresh, retained);
    transfer_single_inner_sliver_state::<WidgetWrapRenderSliver>(fresh, retained);
    transfer_single_inner_sliver_state::<OverlapAbsorberRenderSliver>(fresh, retained);
}

fn adopt_natural_header_state(
    fresh: &mut dyn RenderSliver,
    retained: &mut dyn RenderSliver,
) -> bool {
    let (Some(fresh), Some(retained)) = (
        fresh
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<NaturalHeaderRenderSliver>()),
        retained
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<NaturalHeaderRenderSliver>()),
    ) else {
        return false;
    };
    fresh.adopt_compatible_state(retained)
}

/// Adopts a measured box extent so replacing a viewport descriptor does not
/// flash the lazy default and spuriously move the scroll range. Pinning is
/// presentation-only; the measured extent transfers regardless of it.
fn adopt_box_extent(fresh: &mut dyn RenderSliver, retained: &mut dyn RenderSliver) -> bool {
    let (Some(fresh), Some(retained)) = (
        fresh
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<BoxRenderSliver>()),
        retained
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<BoxRenderSliver>()),
    ) else {
        return false;
    };
    let measured = retained.extent.get();
    if (fresh.extent.get() - measured).abs() <= f32::EPSILON {
        return false;
    }
    fresh.extent.set(measured);
    true
}

/// Adopts a measured floating extent with its reversal tracking so a
/// replacement keeps revealing without replaying hidden distance.
fn adopt_floating_header_state(
    fresh: &mut dyn RenderSliver,
    retained: &mut dyn RenderSliver,
) -> bool {
    let (Some(fresh), Some(retained)) = (
        fresh
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<FloatingHeaderRenderSliver>()),
        retained
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<FloatingHeaderRenderSliver>()),
    ) else {
        return false;
    };
    if (fresh.extent.get() - retained.extent.get()).abs() > f32::EPSILON {
        fresh.extent.set(retained.extent.get());
    }
    fresh.scroll_state = retained.scroll_state;
    if fresh.snap != retained.snap {
        // A running snap belongs to the retained snap configuration. Without
        // an identical flag the fresh header settles to scroll-determined
        // presentation instead of inheriting a foreign animation.
        fresh.scroll_state.cancel_snap();
    }
    true
}

trait SingleInnerSliver {
    fn inner_cell(&mut self) -> &RefCell<Box<dyn RenderSliver>>;
}

impl SingleInnerSliver for PaddingRenderSliver {
    fn inner_cell(&mut self) -> &RefCell<Box<dyn RenderSliver>> {
        &self.inner
    }
}

impl SingleInnerSliver for WidgetWrapRenderSliver {
    fn inner_cell(&mut self) -> &RefCell<Box<dyn RenderSliver>> {
        &self.inner
    }
}

impl SingleInnerSliver for OverlapAbsorberRenderSliver {
    fn inner_cell(&mut self) -> &RefCell<Box<dyn RenderSliver>> {
        &self.inner
    }
}

fn transfer_single_inner_sliver_state<T: SingleInnerSliver + 'static>(
    fresh: &mut dyn RenderSliver,
    retained: &mut dyn RenderSliver,
) {
    if let (Some(fresh), Some(retained)) = (
        fresh.as_any_mut().and_then(|any| any.downcast_mut::<T>()),
        retained
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<T>()),
    ) {
        let mut fresh_inner = fresh.inner_cell().borrow_mut();
        let mut retained_inner = retained.inner_cell().borrow_mut();
        transfer_retained_sliver_state(&mut **fresh_inner, &mut **retained_inner);
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
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        self.sequence.borrow().scroll_layout_dependency()
    }

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

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn invalidate_sliver_child(&self, child: SliverChildId) -> bool {
        let sequence = self.sequence.borrow();
        let Some(sliver) = sequence.children.get(child.scope()) else {
            return false;
        };
        sliver
            .borrow_mut()
            .invalidate_child_measurement(child.local());
        true
    }

    fn adopt_compatible_state(&self, previous: &dyn SliverViewportDelegate) {
        let Some(previous) = previous
            .as_any()
            .and_then(|any| any.downcast_ref::<SequenceViewportDelegate>())
        else {
            return;
        };
        let fresh = self.sequence.borrow_mut();
        let retained = previous.sequence.borrow_mut();
        for (fresh_child, retained_child) in fresh.children.iter().zip(retained.children.iter()) {
            let mut fresh_sliver = fresh_child.borrow_mut();
            let mut retained_sliver = retained_child.borrow_mut();
            transfer_retained_sliver_state(&mut **fresh_sliver, &mut **retained_sliver);
        }
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

    let result = match widget.kind() {
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
            .as_ref()
            .and_then(|child| widget_main_extent_hint(child, axis)),
        WidgetKind::Button(spec) => dimension(spec.size, axis)
            .filter(|extent| *extent > 0.)
            .or_else(|| {
                spec.child
                    .as_ref()
                    .and_then(|child| widget_main_extent_hint(child, axis))
            })
            .or_else(|| dimension(spec.size, axis)),
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
        WidgetKind::TextField(spec) => dimension(spec.size, axis),
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
                constraints.min_height()
            } else {
                constraints.min_width()
            },
            if axis.is_vertical() {
                constraints.max_height()
            } else {
                constraints.max_width()
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
        WidgetKind::Visibility {
            visible,
            hidden,
            child,
        } => {
            if *visible || hidden.layout == crate::tree::HiddenLayout::PreserveSpace {
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
            .as_ref()
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
        .filter_map(|(index, child)| {
            (child.placement == SliverChildPlacement::Pinned).then_some(index)
        })
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
