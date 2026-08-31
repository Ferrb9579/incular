//! Themed, retained Scrollbar control for Incular Controls.
//!
//! A scrollbar is a viewport contract, not a decorative wrapper. The control
//! therefore shares a [`ScrollController`] with the retained
//! [`SingleChildScrollView`] implementation. Wheel input, thumb dragging, track
//! paging, geometry, and painting all continue to be owned by `WidgetTree`.

use crate::theme::ControlTheme;
use incular_scroll::{ScrollController, ScrollbarStyle as RawScrollbarStyle};
use incular_widgets::internal::scroll_view_parts;
use incular_widgets::{SingleChildScrollView, Widget};
use typed_builder::TypedBuilder;

/// Themed vertical scrollbar/scroll-area wrapper.
///
/// `child` is normally the scrollable content. Passing an existing
/// An existing [`SingleChildScrollView`] is also supported; its controller is preserved unless
/// [`Self::controller`] is explicitly supplied. The resulting widget is
/// always a retained scroll viewport, so it participates in the existing
/// wheel, track-click, and thumb-drag input paths.
#[derive(Clone, TypedBuilder)]
pub struct Scrollbar {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(skip))]
    controller: Option<ScrollController>,
    #[builder(default, setter(strip_option))]
    style: Option<RawScrollbarStyle>,
    #[builder(default)]
    thumb_visibility: bool,
}

impl Scrollbar {
    /// Creates a scrollbar around scrollable content.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    /// Uses an externally owned controller, allowing the application to
    /// programmatically scroll and observe the same retained position.
    #[must_use]
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Overrides the theme-derived scrollbar visual tokens.
    #[must_use]
    pub fn style(mut self, style: RawScrollbarStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Keeps the thumb visible even while the pointer is outside the track.
    /// The default is hover reveal; the retained scrollbar remains hit-testable
    /// while hidden so moving over it reveals it before a drag begins.
    #[must_use]
    pub fn thumb_visibility(mut self, visibility: bool) -> Self {
        self.thumb_visibility = visibility;
        self
    }

    /// Builds a retained scroll viewport using the supplied control theme.
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let child = self.child.clone();
        let controller = self.controller.clone();
        let style = self.style;
        let thumb_visibility = self.thumb_visibility;
        let style = style.unwrap_or_else(|| Self::style_from_theme(theme));

        // Keep the original descriptor when no replacement controller is
        // requested. This preserves keys and explicit semantics on an already
        // constructed viewport instead of needlessly erasing its metadata.
        if controller.is_none()
            && let Some((existing_controller, _)) = scroll_view_parts(&child)
        {
            existing_controller.set_scrollbar_style(style);
            existing_controller.set_scrollbar_thumb_visibility(thumb_visibility);
            return child;
        }

        let (controller, child) = match scroll_view_parts(&child) {
            // Reusing an existing viewport should not create nested scroll
            // positions. Rebuild it with the chosen controller so the style
            // and interaction contract stay attached to the visible viewport.
            Some((_, child)) => (controller.unwrap(), child),
            // For ordinary content, retain the complete descriptor (including
            // its key/semantics) as the scroll viewport's child.
            None => (controller.unwrap_or_default(), child),
        };

        controller.set_scrollbar_style(style);
        controller.set_scrollbar_thumb_visibility(thumb_visibility);
        SingleChildScrollView::new(child)
            .controller(controller)
            .into()
    }

    /// Converts ControlTheme colors and metrics into the renderer-independent
    /// scrollbar style consumed by the retained scroll viewport.
    #[must_use]
    pub fn style_from_theme(theme: &ControlTheme) -> RawScrollbarStyle {
        RawScrollbarStyle {
            width: theme.scrollbar.width,
            min_thumb_extent: theme.scrollbar.min_thumb_extent,
            track_color: incular_core::Color::TRANSPARENT,
            thumb_color: theme.colors.foreground_muted,
        }
    }
}

impl From<Scrollbar> for Widget {
    fn from(value: Scrollbar) -> Self {
        let value = std::rc::Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
    }
}
