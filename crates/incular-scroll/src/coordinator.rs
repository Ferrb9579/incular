use crate::{
    controller::ScrollController,
    physics::{ScrollDelta, ScrollPhysics},
};

/// General nested scrolling coordinator. Controllers are ordered innermost to
/// outermost. A clamped child consumes only what it can, and the precise
/// leftover propagates once to its parent.
#[derive(Clone, Debug, Default)]
pub struct NestedScrollCoordinator {
    controllers: Vec<ScrollController>,
    active: Option<usize>,
}

impl NestedScrollCoordinator {
    #[must_use]
    pub fn new(innermost_first: impl IntoIterator<Item = ScrollController>) -> Self {
        Self {
            controllers: innermost_first.into_iter().collect(),
            active: None,
        }
    }

    #[must_use]
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// Transfers a wheel delta without duplication.
    pub fn apply_delta(&mut self, delta: f32) -> ScrollDelta {
        let mut remaining = delta;
        let mut total = 0.;
        self.active = None;
        for (index, controller) in self.controllers.iter().enumerate() {
            let result = ScrollPhysics::clamping().apply_delta(
                controller.offset(),
                remaining,
                0.,
                controller.max_offset(),
            );
            if result.consumed != 0. {
                controller.jump_to(result.position);
                total += result.consumed;
                self.active = Some(index);
            }
            remaining = result.unconsumed;
            if remaining == 0. {
                break;
            }
        }
        ScrollDelta {
            position: total,
            consumed: total,
            unconsumed: remaining,
            accepted: total != 0.,
            ..ScrollDelta::default()
        }
    }

    /// Touch-drag entry point. The sign convention is the same logical
    /// content-offset convention as wheel input.
    pub fn apply_drag_delta(&mut self, delta: f32) -> ScrollDelta {
        self.apply_delta(delta)
    }

    /// Momentum entry point. The coordinator keeps the receiving viewport as
    /// `active_index` until the caller settles or cancels that sequence.
    pub fn apply_momentum_delta(&mut self, delta: f32) -> ScrollDelta {
        self.apply_delta(delta)
    }
}
