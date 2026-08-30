//! Framework defaults shared by application, environment, and widget layers.
//!
//! Defaults that affect more than one crate live here so a constructor cannot
//! silently drift from the runtime or another widget family.  Renderer and
//! design-system policy stays outside this crate: a plain container remains
//! transparent, while a visible application surface belongs to the root
//! widget or an ambient theme.

use crate::{Axis, Brightness, Clip, TextDirection};
use incular_core::Size;

/// Defaults used when an application does not provide window/environment
/// policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApplicationDefaults {
    /// Fallback native window title.
    pub window_title: &'static str,
    /// Initial logical window size before native metrics are known.
    pub initial_window_size: Size,
    pub resizable: bool,
    pub visible: bool,
    pub decorations: bool,
    pub transparent: bool,
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
        resizable: true,
        visible: true,
        decorations: true,
        transparent: false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_defaults_are_valid_for_initialization() {
        let defaults = ApplicationDefaults::DEFAULT;
        assert!(defaults.initial_window_size.width > 0.0);
        assert!(defaults.initial_window_size.height > 0.0);
        assert!(defaults.scale_factor.is_finite() && defaults.scale_factor > 0.0);
        assert!(defaults.text_scale.is_finite() && defaults.text_scale > 0.0);
        assert_eq!(defaults, ApplicationDefaults::default());
    }

    #[test]
    fn widget_defaults_are_explicit_and_safe_for_lazy_layout() {
        let defaults = WidgetDefaults::DEFAULT;
        assert!(defaults.text_size > 0.0);
        assert!(defaults.lazy_item_extent > 0.0);
        assert!(defaults.sliver_cache_extent >= 0.0);
        assert!(defaults.sliver_fill_viewport_extent > 0.0);
        assert_eq!(defaults.grid_cross_axis_count, 1);
        assert_eq!(defaults, WidgetDefaults::default());
    }
}
