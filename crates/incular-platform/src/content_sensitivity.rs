//! Platform-neutral content-capture protection boundaries.
//!
//! Window privacy policy is deliberately expressed as normalized data here.
//! Native adapters decide whether the host can enforce it, while headless
//! embedders can use the in-memory backend to exercise the same state machine.

use incular_config::ContentSensitivity;

/// Whether a native backend can enforce content sensitivity for a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ContentSensitivityCapability {
    /// The backend can apply all normalized sensitivity values.
    Supported,
    /// The backend has no capture-protection primitive for this window.
    #[default]
    Unsupported,
}

/// Why a content-sensitivity request was intentionally not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContentSensitivityNoOpReason {
    /// The native backend does not expose capture-protection support.
    Unsupported,
    /// The requested value was already active.
    Unchanged,
}

/// Result of applying a normalized content-sensitivity request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContentSensitivityOutcome {
    /// The backend accepted and made the requested value active.
    Applied { sensitivity: ContentSensitivity },
    /// The request was observed but did not change native state.
    NoOp {
        requested: ContentSensitivity,
        reason: ContentSensitivityNoOpReason,
    },
}

impl ContentSensitivityOutcome {
    /// Returns the request's normalized value regardless of whether native
    /// state changed.
    #[must_use]
    pub const fn requested(self) -> ContentSensitivity {
        match self {
            Self::Applied { sensitivity } => sensitivity,
            Self::NoOp { requested, .. } => requested,
        }
    }

    /// Returns whether the native backend did not change state.
    #[must_use]
    pub const fn is_noop(self) -> bool {
        matches!(self, Self::NoOp { .. })
    }
}

/// Native capability seam for applying window content sensitivity.
pub trait ContentSensitivityBackend {
    /// Reports support without touching native state.
    fn capability(&self) -> ContentSensitivityCapability;

    /// Applies one normalized request and reports an explicit no-op when the
    /// backend cannot enforce it or the value is already active.
    fn apply(&mut self, sensitivity: ContentSensitivity) -> ContentSensitivityOutcome;
}

/// Explicit unsupported backend used by platforms without a capture policy
/// primitive. Keeping this as a real backend makes unsupported behavior
/// observable and testable rather than silently dropping a command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoopContentSensitivityBackend;

impl ContentSensitivityBackend for NoopContentSensitivityBackend {
    fn capability(&self) -> ContentSensitivityCapability {
        ContentSensitivityCapability::Unsupported
    }

    fn apply(&mut self, sensitivity: ContentSensitivity) -> ContentSensitivityOutcome {
        ContentSensitivityOutcome::NoOp {
            requested: sensitivity,
            reason: ContentSensitivityNoOpReason::Unsupported,
        }
    }
}

/// Deterministic backend for headless hosts and platform-boundary tests.
///
/// A supported instance records every request and reports `Unchanged` for a
/// duplicate request, matching the coalescing expected at a native boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryContentSensitivityBackend {
    capability: ContentSensitivityCapability,
    current: ContentSensitivity,
    outcomes: Vec<ContentSensitivityOutcome>,
}

impl MemoryContentSensitivityBackend {
    /// Creates a backend with the requested capability and a neutral initial
    /// window policy.
    #[must_use]
    pub fn new(capability: ContentSensitivityCapability) -> Self {
        Self {
            capability,
            current: ContentSensitivity::NotSensitive,
            outcomes: Vec::new(),
        }
    }

    /// Creates a backend that accepts capture-protection requests.
    #[must_use]
    pub fn supported() -> Self {
        Self::new(ContentSensitivityCapability::Supported)
    }

    /// Creates a backend that reports every request as unsupported.
    #[must_use]
    pub fn unsupported() -> Self {
        Self::new(ContentSensitivityCapability::Unsupported)
    }

    /// Returns the most recently applied supported value.
    #[must_use]
    pub const fn current(&self) -> ContentSensitivity {
        self.current
    }

    /// Returns all outcomes in request order.
    #[must_use]
    pub fn outcomes(&self) -> &[ContentSensitivityOutcome] {
        &self.outcomes
    }

    /// Clears recorded outcomes without changing the simulated native value.
    pub fn clear_outcomes(&mut self) {
        self.outcomes.clear();
    }
}

impl Default for MemoryContentSensitivityBackend {
    fn default() -> Self {
        Self::supported()
    }
}

impl ContentSensitivityBackend for MemoryContentSensitivityBackend {
    fn capability(&self) -> ContentSensitivityCapability {
        self.capability
    }

    fn apply(&mut self, sensitivity: ContentSensitivity) -> ContentSensitivityOutcome {
        let outcome = match self.capability {
            ContentSensitivityCapability::Unsupported => ContentSensitivityOutcome::NoOp {
                requested: sensitivity,
                reason: ContentSensitivityNoOpReason::Unsupported,
            },
            ContentSensitivityCapability::Supported if self.current == sensitivity => {
                ContentSensitivityOutcome::NoOp {
                    requested: sensitivity,
                    reason: ContentSensitivityNoOpReason::Unchanged,
                }
            }
            ContentSensitivityCapability::Supported => {
                self.current = sensitivity;
                ContentSensitivityOutcome::Applied { sensitivity }
            }
        };
        self.outcomes.push(outcome);
        outcome
    }
}
