use crate::{
    controller::ScrollController, notifications::ScrollNotificationType,
    restoration::persist_scroll_offset,
};

/// The default clamping policy used by native desktop/mobile views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClampingScrollPhysics;
impl ClampingScrollPhysics {
    #[must_use]
    pub fn apply(self, current: f32, delta: f32, min: f32, max: f32) -> f32 {
        (current + delta).clamp(min.min(max), max.max(min))
    }
}

/// Whether a policy participates in a gesture when there is no scroll range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scrollability {
    /// Accept a gesture only when a position can move.
    #[default]
    WhenScrollable,
    /// Accept a gesture even for a zero-length range (useful for pull effects).
    Always,
    /// Never consume a gesture.
    Never,
}

/// Boundary behavior for the unified [`ScrollPhysics`] policy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BoundaryPhysics {
    /// Clamp at the content bounds and return the unused delta to a parent
    /// coordinator.
    #[default]
    Clamping,
    /// Permit a finite, resistant visual overscroll. Call
    /// [`ScrollPhysics::spring_step`] with monotonic frame deltas to return it
    /// to the nearest content bound.
    Bouncing {
        resistance: f32,
        max_overscroll: f32,
        spring: f32,
        damping: f32,
    },
}

/// Optional deterministic settle target after wheel/drag momentum ends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SnapPhysics {
    #[default]
    None,
    Page {
        extent: f32,
    },
    /// A page whose extent is the viewport's current main-axis dimension.
    /// This is the policy used by a Flutter-style `PageView` with static
    /// children; unlike a fixed page extent it remains correct after a window
    /// resize.
    PageViewport,
    FixedExtent {
        extent: f32,
    },
}

/// Result of one policy application. `unconsumed` is deliberately explicit so
/// nested viewports transfer a delta once, rather than applying it to every
/// ancestor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollDelta {
    pub position: f32,
    pub consumed: f32,
    pub unconsumed: f32,
    pub overscroll: f32,
    pub accepted: bool,
}

/// One monotonic-frame spring integration result for bouncing scroll.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollSpringStep {
    pub position: f32,
    pub velocity: f32,
    pub settled: bool,
}

/// Rust-native composable scroll policy. It replaces a hierarchy of widget
/// subclasses with independent scrollability, boundary, and snap choices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollPhysics {
    pub scrollability: Scrollability,
    pub boundary: BoundaryPhysics,
    pub snap: SnapPhysics,
    maintain_range: bool,
}

impl Default for ScrollPhysics {
    fn default() -> Self {
        Self::clamping()
    }
}

impl ScrollPhysics {
    #[must_use]
    pub const fn clamping() -> Self {
        Self {
            scrollability: Scrollability::WhenScrollable,
            boundary: BoundaryPhysics::Clamping,
            snap: SnapPhysics::None,
            maintain_range: false,
        }
    }

    #[must_use]
    pub const fn always_scrollable(mut self) -> Self {
        self.scrollability = Scrollability::Always;
        self
    }

    #[must_use]
    pub const fn never_scrollable(mut self) -> Self {
        self.scrollability = Scrollability::Never;
        self
    }

    #[must_use]
    pub const fn bouncing(mut self) -> Self {
        self.boundary = BoundaryPhysics::Bouncing {
            resistance: 0.5,
            max_overscroll: 160.,
            spring: 220.,
            damping: 28.,
        };
        self
    }

    #[must_use]
    pub const fn page_snapping(mut self, extent: f32) -> Self {
        self.snap = SnapPhysics::Page { extent };
        self
    }

    /// Enables viewport-sized page snapping.  This is the composable
    /// equivalent of Flutter's `PageScrollPhysics` when the page extent is
    /// supplied by the viewport rather than a fixed constructor argument.
    #[must_use]
    pub const fn page(mut self) -> Self {
        self.snap = SnapPhysics::PageViewport;
        self
    }

    #[must_use]
    pub const fn fixed_extent_snapping(mut self, extent: f32) -> Self {
        self.snap = SnapPhysics::FixedExtent { extent };
        self
    }

    /// Applies a raw logical delta. Bounds are normalized, non-finite input is
    /// rejected, and clamping returns exactly the unused portion for nesting.
    #[must_use]
    pub fn apply_delta(self, current: f32, delta: f32, min: f32, max: f32) -> ScrollDelta {
        if !current.is_finite() || !delta.is_finite() {
            return ScrollDelta {
                position: current,
                unconsumed: delta,
                ..ScrollDelta::default()
            };
        }
        if self.scrollability == Scrollability::Never {
            return ScrollDelta {
                position: current,
                unconsumed: delta,
                ..ScrollDelta::default()
            };
        }
        let (min, max) = (min.min(max), max.max(min));
        if min == max && self.scrollability == Scrollability::WhenScrollable {
            return ScrollDelta {
                position: current.clamp(min, max),
                unconsumed: delta,
                ..ScrollDelta::default()
            };
        }
        match self.boundary {
            BoundaryPhysics::Clamping => {
                let position = (current + delta).clamp(min, max);
                let consumed = position - current;
                ScrollDelta {
                    position,
                    consumed,
                    unconsumed: delta - consumed,
                    accepted: consumed != 0. || self.scrollability == Scrollability::Always,
                    ..ScrollDelta::default()
                }
            }
            BoundaryPhysics::Bouncing {
                resistance,
                max_overscroll,
                ..
            } => {
                let resistance = resistance.clamp(0., 1.);
                let limit = max_overscroll.max(0.);
                let raw = current + delta;
                let position = if raw < min {
                    (min + (raw - min) * resistance).max(min - limit)
                } else if raw > max {
                    (max + (raw - max) * resistance).min(max + limit)
                } else {
                    raw
                };
                let consumed = position - current;
                ScrollDelta {
                    position,
                    consumed,
                    // A resistant boundary owns a sequence (including its
                    // finite visual limit); callers return it with the spring
                    // rather than duplicating the same delta into a parent.
                    unconsumed: 0.,
                    overscroll: if position < min {
                        position - min
                    } else {
                        (position - max).max(0.)
                    },
                    accepted: true,
                }
            }
        }
    }

    /// Selects the deterministic page/fixed-item settle target from current
    /// position and velocity. A positive velocity advances, a negative one
    /// retreats; low velocity picks the nearest item.
    #[must_use]
    pub fn snap_target(self, position: f32, velocity: f32, min: f32, max: f32) -> f32 {
        self.snap_target_for_extent(position, velocity, min, max, 0.)
    }

    /// Selects a snap target, resolving [`SnapPhysics::PageViewport`] against
    /// the supplied viewport extent.
    #[must_use]
    pub fn snap_target_for_extent(
        self,
        position: f32,
        velocity: f32,
        min: f32,
        max: f32,
        viewport_extent: f32,
    ) -> f32 {
        let extent = match self.snap {
            SnapPhysics::None => return position.clamp(min.min(max), max.max(min)),
            SnapPhysics::Page { extent } | SnapPhysics::FixedExtent { extent } => extent,
            SnapPhysics::PageViewport => viewport_extent,
        };
        if !extent.is_finite() || extent <= 0. {
            return position.clamp(min.min(max), max.max(min));
        }
        let unit = position / extent;
        let index = if velocity > 120. {
            unit.ceil()
        } else if velocity < -120. {
            unit.floor()
        } else {
            unit.round()
        };
        (index * extent).clamp(min.min(max), max.max(min))
    }

    /// Returns the page/fixed-item step used for semantic scroll actions.
    /// Viewport pages resolve against the current viewport dimension.
    #[must_use]
    pub fn snap_extent(self, viewport_extent: f32) -> Option<f32> {
        let extent = match self.snap {
            SnapPhysics::Page { extent } | SnapPhysics::FixedExtent { extent } => extent,
            SnapPhysics::PageViewport => viewport_extent,
            SnapPhysics::None => return None,
        };
        (extent.is_finite() && extent > 0.).then_some(extent)
    }

    /// Advances a bounce spring using a monotonic elapsed frame duration in
    /// seconds. Runtime schedulers own when to call this; no sleep or executor
    /// timing is hidden in the policy.
    #[must_use]
    pub fn spring_step(
        self,
        position: f32,
        velocity: f32,
        elapsed_seconds: f32,
        min: f32,
        max: f32,
    ) -> ScrollSpringStep {
        let BoundaryPhysics::Bouncing {
            spring, damping, ..
        } = self.boundary
        else {
            return ScrollSpringStep {
                position: position.clamp(min.min(max), max.max(min)),
                velocity: 0.,
                settled: true,
            };
        };
        let target = position.clamp(min.min(max), max.max(min));
        let dt = elapsed_seconds.clamp(0., 0.05);
        let next_velocity = (velocity + (target - position) * spring * dt) * (-damping * dt).exp();
        let next_position = position + next_velocity * dt;
        let settled = (next_position - target).abs() < 0.01 && next_velocity.abs() < 0.01;
        ScrollSpringStep {
            position: if settled { target } else { next_position },
            velocity: if settled { 0. } else { next_velocity },
            settled,
        }
    }

    #[must_use]
    pub fn parent(mut self, parent: Self) -> Self {
        if self.scrollability == Scrollability::WhenScrollable {
            self.scrollability = parent.scrollability;
        }
        if self.boundary == BoundaryPhysics::Clamping {
            self.boundary = parent.boundary;
        }
        if self.snap == SnapPhysics::None {
            self.snap = parent.snap;
        }
        self.maintain_range |= parent.maintain_range;
        self
    }

    #[must_use]
    pub fn then(self, next: Self) -> Self {
        next.parent(self)
    }

    #[must_use]
    pub const fn range_maintaining(mut self) -> Self {
        self.maintain_range = true;
        self
    }

    /// Whether this policy keeps a trailing-edge position when extents
    /// change.
    #[must_use]
    pub const fn is_range_maintaining(self) -> bool {
        self.maintain_range
    }

    #[must_use]
    pub const fn carousel(mut self, item_extent: f32) -> Self {
        self.snap = SnapPhysics::FixedExtent {
            extent: item_extent,
        };
        self
    }

    #[must_use]
    pub const fn min_fling_velocity(&self) -> f32 {
        50.0
    }

    #[must_use]
    pub const fn max_fling_velocity(&self) -> f32 {
        8000.0
    }

    #[must_use]
    pub const fn drag_start_distance_motion_threshold(&self) -> f32 {
        3.5
    }
}

impl ScrollController {
    /// Applies one unified scroll policy step. Clamping is persisted normally;
    /// transient bounce positions stay in memory and return through
    /// [`ScrollPhysics::spring_step`] rather than being restored as a logical
    /// position.
    pub fn apply_physics(&self, physics: ScrollPhysics, delta: f32) -> ScrollDelta {
        let previous = self.offset();
        let result = physics.apply_delta(previous, delta, 0., self.max_offset());
        if result.position == previous {
            if result.overscroll != 0. {
                self.dispatch_notification(
                    ScrollNotificationType::Overscroll,
                    0.,
                    result.overscroll,
                );
            }
            return result;
        }
        let persistence = {
            let mut state = self.state.borrow_mut();
            state.offset = result.position;
            state.pending_restored_offset = None;
            state.revision += 1;
            matches!(physics.boundary, BoundaryPhysics::Clamping)
                .then(|| (state.restoration.clone(), result.position))
        };
        if let Some((restoration, offset)) = persistence {
            persist_scroll_offset(restoration, offset);
        }
        self.dispatch_notification(
            ScrollNotificationType::Update,
            result.position - previous,
            0.,
        );
        if result.overscroll != 0. {
            self.dispatch_notification(
                ScrollNotificationType::Overscroll,
                result.position - previous,
                result.overscroll,
            );
        }
        result
    }

    /// Stores a spring result produced with a monotonic frame duration.
    pub fn apply_spring_step(&self, step: ScrollSpringStep) -> bool {
        let previous = self.offset();
        if !step.position.is_finite() || step.position == previous {
            return false;
        }
        let mut state = self.state.borrow_mut();
        state.offset = step.position;
        state.revision += 1;
        drop(state);
        self.dispatch_notification(ScrollNotificationType::Update, step.position - previous, 0.);
        true
    }
    /// Updates content and viewport extents after a viewport layout pass.
    ///
    /// The method is public so independent viewport implementations can share
    /// a controller; applications normally use `jump_to` or `scroll_by`.
    pub fn update_extents(&self, content: f32, viewport: f32) {
        self.update_extents_with_physics(content, viewport, ScrollPhysics::default());
    }

    /// Updates content and viewport extents while applying the configured
    /// range-maintaining policy.
    ///
    /// A plain extent update clamps a position when the new range becomes
    /// smaller.  [`ScrollPhysics::range_maintaining`] additionally keeps a
    /// position that was anchored to the old trailing edge anchored to the
    /// new trailing edge.  This is the important case for a list whose
    /// content grows, shrinks, or whose viewport is resized while the user is
    /// at the end.  The ordinary `update_extents` API remains available for
    /// callers that want simple clamping.
    pub fn update_extents_with_physics(&self, content: f32, viewport: f32, physics: ScrollPhysics) {
        let (old_offset, old_content, old_viewport) = {
            let state = self.state.borrow();
            (state.offset, state.content_extent, state.viewport_extent)
        };
        let persistence = {
            let mut state = self.state.borrow_mut();
            let old_max_offset = state.max_offset;
            let old_offset = state.offset;
            state.content_extent = content.max(0.);
            state.viewport_extent = viewport.max(0.);
            state.max_offset = (state.content_extent - state.viewport_extent).max(0.);
            if let Some(requested) = state.pending_jump_offset {
                let next = requested.min(state.max_offset);
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                }
                if requested <= state.max_offset {
                    state.pending_jump_offset = None;
                }
                None
            } else if let Some(restored) = state.pending_restored_offset {
                let next = restored.min(state.max_offset);
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                }
                // Keep a larger requested position pending while asynchronous
                // content grows, instead of overwriting the only snapshot
                // with a temporary short-content clamp.
                if restored <= state.max_offset {
                    state.pending_restored_offset = None;
                }
                None
            } else {
                let next = if physics.is_range_maintaining()
                    && (state.max_offset - old_max_offset).abs() > f32::EPSILON
                    && old_max_offset > 0.
                    && (old_offset - old_max_offset).abs() <= 0.001
                {
                    // Preserve the logical trailing-edge anchor when the
                    // range changes.  This handles both content mutation and
                    // viewport resize without rebuilding the scroll view.
                    state.max_offset
                } else {
                    old_offset.clamp(0., state.max_offset)
                };
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                    Some((state.restoration.clone(), next))
                } else {
                    None
                }
            }
        };
        if let Some((restoration, offset)) = persistence {
            persist_scroll_offset(restoration, offset);
        }
        let next_offset = self.offset();
        if (content.max(0.) - old_content).abs() > f32::EPSILON
            || (viewport.max(0.) - old_viewport).abs() > f32::EPSILON
        {
            self.dispatch_notification(ScrollNotificationType::Metrics, 0., 0.);
        }
        if (next_offset - old_offset).abs() > f32::EPSILON {
            self.dispatch_notification(
                ScrollNotificationType::Update,
                next_offset - old_offset,
                0.,
            );
        }
    }

    /// Settles this position according to a composed physics policy.
    ///
    /// Bouncing positions return to the nearest range boundary, while page or
    /// fixed-extent policies choose their deterministic snap target.  The
    /// operation is intentionally synchronous; the runtime can animate toward
    /// the returned target using its normal frame scheduler when desired.
    pub fn settle_physics(&self, physics: ScrollPhysics, velocity: f32) -> bool {
        let target = physics.snap_target_for_extent(
            self.offset(),
            velocity,
            0.,
            self.max_offset(),
            self.viewport_extent(),
        );
        self.jump_to(target)
    }

    /// Preserves the visible logical anchor after content is inserted or
    /// removed before the current viewport. `delta_before_viewport` is the
    /// change in content extent before the anchor (positive for insertion,
    /// negative for removal). This is intentionally an explicit mutation
    /// operation so virtualized models can call it without forcing a rebuild.
    pub fn adjust_for_content_change(
        &self,
        delta_before_viewport: f32,
        physics: ScrollPhysics,
    ) -> bool {
        if !physics.is_range_maintaining() || !delta_before_viewport.is_finite() {
            return false;
        }
        let current = self.offset();
        self.jump_to(current + delta_before_viewport)
    }
}
