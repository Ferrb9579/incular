//! Framework defaults shared by application, environment, and widget layers.
//!
//! Defaults that affect more than one crate live here so a constructor cannot
//! silently drift from the runtime or another widget family.  Renderer and
//! design-system policy stays outside this crate: a plain container remains
//! transparent, while a visible application surface belongs to the root
//! widget or an ambient theme.

use crate::{Axis, Brightness, Clip, TextDirection};
use incular_core::{Color, Size};

/// Whether a native application window participates in desktop compositing
/// with framebuffer alpha.
///
/// This is presentation policy, not a paint color. A transparent window may
/// render fully opaque frames, and an opaque window may still use alpha inside
/// its render tree for antialiasing, effects, and offscreen composition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransparencyMode {
    #[default]
    Opaque,
    Transparent,
}

impl TransparencyMode {
    #[must_use]
    pub const fn is_transparent(self) -> bool {
        matches!(self, Self::Transparent)
    }
}

/// Which side of the content/window relationship owns the logical size.
///
/// `Viewport` is the normal application-window model: native window metrics
/// are authoritative and the root receives tight viewport constraints.
/// `Content` is intended for compact, content-sized top-level windows: the
/// initial logical size is the stable minimum content viewport, retained content may
/// grow beyond it, and the native host follows the measured root layout extent.
/// Paint-only overflow and transient presentation are not content size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowSizePolicy {
    #[default]
    Viewport,
    Content,
}

/// Defaults used when an application does not provide window/environment
/// policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApplicationDefaults {
    /// Fallback native window title.
    pub window_title: &'static str,
    /// Initial logical window size before native metrics are known.
    pub initial_window_size: Size,
    pub window_size_policy: WindowSizePolicy,
    pub resizable: bool,
    pub visible: bool,
    pub decorations: bool,
    /// How the native host/view participates in operating-system compositing.
    pub transparency_mode: TransparencyMode,
    /// Base color underneath the application's rendered scene.
    ///
    /// This is independent of [`TransparencyMode`]. A transparent host may use
    /// an opaque background, and an opaque host may begin from transparent
    /// scene content before native presentation.
    pub background_color: Color,
    pub maximized: bool,
    /// Neutral environment values used until a platform publishes a snapshot.
    pub scale_factor: f64,
    pub text_scale: f32,
    pub brightness: Brightness,
    pub text_direction: TextDirection,
    pub reduced_motion: bool,
}

impl ApplicationDefaults {
    /// The framework's stable desktop/application defaults.
    pub const DEFAULT: Self = Self {
        window_title: "Incular",
        initial_window_size: Size {
            width: 800.0,
            height: 600.0,
        },
        window_size_policy: WindowSizePolicy::Viewport,
        resizable: true,
        visible: true,
        decorations: true,
        transparency_mode: TransparencyMode::Opaque,
        background_color: Color::BLACK,
        maximized: false,
        scale_factor: 1.0,
        text_scale: 1.0,
        brightness: Brightness::Light,
        text_direction: TextDirection::Ltr,
        reduced_motion: false,
    };
}

impl Default for ApplicationDefaults {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Defaults shared by plain text and sliver-backed widgets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetDefaults {
    /// Font size used by a plain `Text`/`TextStyle` with no ambient override.
    pub text_size: f32,
    /// Conservative estimate used before a lazy child has been laid out.
    pub lazy_item_extent: f32,
    /// Logical pixels materialized before and after a sliver viewport.
    pub sliver_cache_extent: f32,
    /// Fallback main-axis extent for an unbounded `SliverFillViewport`.
    pub sliver_fill_viewport_extent: f32,
    pub scroll_direction: Axis,
    pub page_scroll_direction: Axis,
    pub scroll_clip_behavior: Clip,
    pub grid_cross_axis_count: usize,
}

impl WidgetDefaults {
    /// The framework's stable widget defaults.
    pub const DEFAULT: Self = Self {
        text_size: 16.0,
        lazy_item_extent: 48.0,
        sliver_cache_extent: 250.0,
        sliver_fill_viewport_extent: 600.0,
        scroll_direction: Axis::Vertical,
        page_scroll_direction: Axis::Horizontal,
        scroll_clip_behavior: Clip::HardEdge,
        grid_cross_axis_count: 1,
    };
}

impl Default for WidgetDefaults {
    fn default() -> Self {
        Self::DEFAULT
    }
}
