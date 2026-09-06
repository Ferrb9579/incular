//! Native per-window ownership. Raw-handle users precede the retained window.
use crate::{
    environment::DesktopEnvironmentProvider, external_drag::ExternalFileDragState,
    input::WindowInputState, pointer::NativeCursorCoordinator,
};
use accesskit_winit::Adapter as AccessKitAdapter;
use incular_accessibility::AccessKitProjection;
use incular_platform::{NoopContentSensitivityBackend, WindowId as IncularWindowId, WindowMetrics};
use incular_wgpu::WgpuRenderer;
use std::sync::Arc;
use winit::window::Window;
/// One AccessKit adapter/projection pair belongs to exactly one native window.
/// The projection is pure retained-tree state; `Adapter` owns the OS bridge.
pub(crate) struct NativeAccessibilityState {
    pub(crate) adapter: AccessKitAdapter,
    pub(crate) projection: AccessKitProjection,
    pub(crate) active: bool,
}
pub(crate) struct NativeWindowState {
    pub(crate) id: IncularWindowId,
    pub(crate) renderer: WgpuRenderer,
    pub(crate) metrics: WindowMetrics,
    pub(crate) input: WindowInputState,
    pub(crate) native_cursor: NativeCursorCoordinator,
    pub(crate) environment: DesktopEnvironmentProvider,
    pub(crate) external_file_drag: ExternalFileDragState,
    pub(crate) accessibility: NativeAccessibilityState,
    pub(crate) content_sensitivity: NoopContentSensitivityBackend,
    /// Must outlive every field that was created from this window's raw
    /// handles. Struct fields drop in declaration order, so keep it last.
    pub(crate) window: Arc<Window>,
}
