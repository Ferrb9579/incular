use std::collections::HashMap;
use std::rc::Rc;

use incular_core::{Offset, PointerPhase};

use crate::details::PointerEvent;

/// Metadata for scale start events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleStartDetails {
    pub focal_point: Offset,
    pub pointer_count: usize,
}

/// The current geometry of a two-pointer scale interaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleUpdateDetails {
    pub focal_point: Offset,
    pub scale: f32,
    pub pointer_count: usize,
}

/// Metadata for scale end events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleEndDetails {
    pub pointer_count: usize,
}

impl ScaleUpdateDetails {
    #[must_use]
    pub const fn new(focal_point: Offset, scale: f32) -> Self {
        Self {
            focal_point,
            scale,
            pointer_count: 2,
        }
    }
}

/// A platform-neutral two-pointer scale recognizer.
pub struct ScaleGestureDetector {
    pointers: HashMap<u64, Offset>,
    initial_distance: Option<f32>,
    on_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
    on_start: Option<Rc<dyn Fn(ScaleStartDetails)>>,
    on_end: Option<Rc<dyn Fn(ScaleEndDetails)>>,
    started: bool,
}
impl ScaleGestureDetector {
    #[must_use]
    pub fn new(on_update: impl Fn(ScaleUpdateDetails) + 'static) -> Self {
        Self::with_callbacks(Some(Rc::new(on_update)), None, None)
    }

    /// Creates a scale recognizer with the complete Flutter-style callback
    /// lifecycle. `new` remains the compact update-only constructor.
    #[must_use]
    pub fn with_callbacks(
        on_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
        on_start: Option<Rc<dyn Fn(ScaleStartDetails)>>,
        on_end: Option<Rc<dyn Fn(ScaleEndDetails)>>,
    ) -> Self {
        Self {
            pointers: HashMap::new(),
            initial_distance: None,
            on_update,
            on_start,
            on_end,
            started: false,
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        let handled = match event.phase {
            PointerPhase::Down => true,
            PointerPhase::Move => self.pointers.contains_key(&event.pointer),
            PointerPhase::Up | PointerPhase::Cancel => self.pointers.contains_key(&event.pointer),
        };
        if let Some(details) = self.observe(event) {
            self.dispatch(details);
        }
        handled
    }
    /// Updates internal contact geometry without invoking the user callback.
    /// Retained adapters use this to wait until the scale recognizer wins its
    /// arena before exposing any scale updates.
    pub fn observe(&mut self, event: PointerEvent) -> Option<ScaleUpdateDetails> {
        match event.phase {
            PointerPhase::Down => {
                self.pointers.insert(event.pointer, event.position);
                self.reset_initial_distance();
                None
            }
            PointerPhase::Move => {
                let position = self.pointers.get_mut(&event.pointer)?;
                *position = event.position;
                let (first, second) = self.first_two()?;
                let distance = distance(first, second);
                let initial = self
                    .initial_distance
                    .get_or_insert(distance.max(f32::EPSILON));
                let details = ScaleUpdateDetails::new(
                    Offset::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5),
                    distance / *initial,
                );
                if !self.started {
                    self.started = true;
                    if let Some(callback) = &self.on_start {
                        callback(ScaleStartDetails {
                            focal_point: details.focal_point,
                            pointer_count: details.pointer_count,
                        });
                    }
                }
                Some(details)
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                if self.started {
                    self.started = false;
                    if let Some(callback) = &self.on_end {
                        callback(ScaleEndDetails {
                            pointer_count: self.pointers.len(),
                        });
                    }
                }
                self.pointers.remove(&event.pointer);
                self.reset_initial_distance();
                None
            }
        }
    }
    /// Delivers a previously observed update after the caller has resolved
    /// arbitration for the relevant pointer stream.
    pub fn dispatch(&self, details: ScaleUpdateDetails) {
        if let Some(callback) = &self.on_update {
            callback(details);
        }
    }
    fn first_two(&self) -> Option<(Offset, Offset)> {
        let mut pointers = self.pointers.values().copied();
        Some((pointers.next()?, pointers.next()?))
    }
    fn reset_initial_distance(&mut self) {
        self.initial_distance = self
            .first_two()
            .map(|(first, second)| distance(first, second));
    }
}

fn distance(first: Offset, second: Offset) -> f32 {
    (first.x - second.x).hypot(first.y - second.y)
}
