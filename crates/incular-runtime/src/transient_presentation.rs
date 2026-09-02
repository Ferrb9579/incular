use incular_config::{TransientPresentation, TransientRole};
use incular_widgets::TransientSurfaceId;

/// Presentation actually selected by the native adapter for one retained
/// transient during its current visible lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedTransientPresentation {
    Native,
    Overlay,
}

/// Why an `Auto` transient is currently using the owning view's overlay.
///
/// These categories are deliberately renderer/platform neutral. Native error
/// objects remain at the adapter boundary; diagnostics only need a stable
/// explanation of the fallback decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransientFallbackReason {
    /// Application code explicitly requested `TransientPresentation::Overlay`.
    RequestedOverlay,
    /// The active backend/session does not support this transient role.
    NativeUnsupported,
    /// Native host creation, attachment, presentation, or rendering failed.
    NativeHostUnavailable,
    /// The native swapchain cannot preserve required transparent pixels for
    /// this retained popup. Falling back avoids visually corrupt opaque corners.
    SurfaceTransparencyRequired,
}

/// Read-only resolved state for one visible semantic transient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransientPresentationResolution {
    pub id: TransientSurfaceId,
    pub role: TransientRole,
    pub requested: TransientPresentation,
    pub resolved: ResolvedTransientPresentation,
    pub fallback_reason: Option<TransientFallbackReason>,
}
