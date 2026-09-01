//! Portable native-operation identity, completion, and errors.

use crate::WindowId;
use std::fmt;

/// Stable identity for one result-bearing native request.
///
/// IDs are allocated by the runtime command bridge and are never native
/// command IDs or handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NativeRequestId(u64);

impl NativeRequestId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable category for a failure at the native platform boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlatformOperationErrorKind {
    Unsupported,
    Unavailable,
    RejectedByPlatform,
    InvalidState,
    StaleResource,
    NativeFailure,
}

/// Backend-neutral native operation failure.
///
/// `context` is diagnostic text only. Public code must branch on [`Self::kind`]
/// rather than parsing platform error strings.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlatformOperationError {
    kind: PlatformOperationErrorKind,
    context: Option<String>,
}

impl PlatformOperationError {
    #[must_use]
    pub const fn new(kind: PlatformOperationErrorKind) -> Self {
        Self {
            kind,
            context: None,
        }
    }

    #[must_use]
    pub fn with_context(kind: PlatformOperationErrorKind, context: impl Into<String>) -> Self {
        Self {
            kind,
            context: Some(context.into()),
        }
    }

    #[must_use]
    pub const fn unsupported() -> Self {
        Self::new(PlatformOperationErrorKind::Unsupported)
    }

    #[must_use]
    pub const fn unavailable() -> Self {
        Self::new(PlatformOperationErrorKind::Unavailable)
    }

    #[must_use]
    pub const fn stale_resource() -> Self {
        Self::new(PlatformOperationErrorKind::StaleResource)
    }

    #[must_use]
    pub const fn kind(&self) -> PlatformOperationErrorKind {
        self.kind
    }

    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }
}

impl fmt::Display for PlatformOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let category = match self.kind {
            PlatformOperationErrorKind::Unsupported => "operation is unsupported",
            PlatformOperationErrorKind::Unavailable => "platform service is unavailable",
            PlatformOperationErrorKind::RejectedByPlatform => {
                "operation was rejected by the platform"
            }
            PlatformOperationErrorKind::InvalidState => "operation is invalid in the current state",
            PlatformOperationErrorKind::StaleResource => "target resource is stale",
            PlatformOperationErrorKind::NativeFailure => "native operation failed",
        };
        match &self.context {
            Some(context) => write!(formatter, "{category}: {context}"),
            None => formatter.write_str(category),
        }
    }
}

impl std::error::Error for PlatformOperationError {}

pub type PlatformOperationResult = Result<(), PlatformOperationError>;

/// One native backend completion routed back to the runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOperationCompletion {
    pub window_id: WindowId,
    pub request_id: NativeRequestId,
    pub result: PlatformOperationResult,
}

impl NativeOperationCompletion {
    #[must_use]
    pub const fn new(
        window_id: WindowId,
        request_id: NativeRequestId,
        result: PlatformOperationResult,
    ) -> Self {
        Self {
            window_id,
            request_id,
            result,
        }
    }
}
