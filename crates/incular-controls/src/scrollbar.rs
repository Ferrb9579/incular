//! Themed, retained Scrollbar control for Incular Controls.
//!
//! A scrollbar is a viewport contract, not a decorative wrapper. The control
//! therefore shares a [`ScrollController`] with the retained
//! `Widget::scroll_view` implementation. Wheel input, thumb dragging, track
//! paging, geometry, and painting all continue to be owned by `WidgetTree`.

use crate::theme::ControlTheme;
use incular_scroll::{ScrollController, ScrollbarStyle as RawScrollbarStyle};
use incular_widgets::Widget;
use incular_widgets::internal::WidgetKind;
use typed_builder::TypedBuilder;

/// Themed vertical scrollbar/scroll-area wrapper.
///
/// `child` is normally the scrollable content. Passing an existing
/// `Widget::scroll_view` is also supported; its controller is preserved unless
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
            && let WidgetKind::Scroll { controller, .. } = &child.kind
        {
            controller.set_scrollbar_style(style);
            controller.set_scrollbar_thumb_visibility(thumb_visibility);
            return child;
        }

        let (controller, child) = match child.kind {
            // Reusing an existing viewport should not create nested scroll
            // positions. Rebuild it with the chosen controller so the style
            // and interaction contract stay attached to the visible viewport.
            WidgetKind::Scroll { child, .. } => (controller.unwrap(), *child),
            // For ordinary content, retain the complete descriptor (including
            // its key/semantics) as the scroll viewport's child.
            _ => (controller.unwrap_or_default(), child),
        };

        controller.set_scrollbar_style(style);
        controller.set_scrollbar_thumb_visibility(thumb_visibility);
        Widget::scroll_view(controller, child)
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
        Widget::layout_builder(move |_| {
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_config::Constraints;
    use incular_core::{Color, Offset, Size};
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::Text;
    use incular_widgets::internal::WidgetTree;

    #[test]
    fn builder_keeps_controller_private_and_uses_explicit_defaults() {
        let default = Scrollbar::builder().child(Text::new("content")).build();
        assert!(default.controller.is_none());
        assert!(default.style.is_none());
        assert!(!default.thumb_visibility);

        let style = RawScrollbarStyle {
            width: 10.,
            min_thumb_extent: 24.,
            track_color: Color::TRANSPARENT,
            thumb_color: Color::WHITE,
        };
        let configured = Scrollbar::builder()
            .child(Text::new("content"))
            .style(style)
            .thumb_visibility(true)
            .build();
        assert_eq!(configured.style, Some(style));
        assert!(configured.thumb_visibility);
    }

    #[test]
    fn plain_content_becomes_a_retained_scroll_view() {
        let controller = ScrollController::new();
        let widget = Scrollbar::new(Widget::fixed_box(Size::new(120., 1_000.), Color::WHITE))
            .controller(controller.clone())
            .thumb_visibility(true)
            .build(&ControlTheme::dark());

        assert!(matches!(widget.kind, WidgetKind::Scroll { .. }));

        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount scrollbar");
        tree.layout(Constraints::tight(Size::new(120., 100.)));

        let geometry = tree
            .scrollbar_diagnostics()
            .pop()
            .expect("retained scrollbar geometry");
        assert!(geometry.visible);
        assert_eq!(geometry.track.size.width, 8.);

        // The global retained scrollbar path owns the same controller used by
        // the wrapper, proving pointer interaction is not decorative.
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(115., 10.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(115., 65.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(115., 65.)));
        assert!(controller.offset() > 0.);
    }

    #[test]
    fn themed_style_reaches_retained_track_and_thumb_paint() {
        let controller = ScrollController::new();
        let style = RawScrollbarStyle {
            width: 12.,
            min_thumb_extent: 30.,
            track_color: Color::rgba(10, 20, 30, 120),
            thumb_color: Color::rgba(200, 210, 220, 230),
        };
        let widget = Scrollbar::new(Widget::fixed_box(Size::new(100., 800.), Color::WHITE))
            .controller(controller)
            .style(style)
            .thumb_visibility(true);
        let mut tree = WidgetTree::new();
        tree.mount(widget.into()).expect("mount scrollbar");
        tree.layout(Constraints::tight(Size::new(100., 100.)));

        let commands = tree.paint();
        assert!(commands.commands().iter().any(|command| {
            matches!(command, PaintCommand::RRect { rrect, brush } if rrect.rect.size.width == 12. && brush == &Brush::Solid(style.track_color))
        }));
        assert!(commands.commands().iter().any(|command| {
            matches!(command, PaintCommand::RRect { rrect, brush } if rrect.rect.size.width == 12. && brush == &Brush::Solid(style.thumb_color))
        }));
    }

    #[test]
    fn existing_scroll_view_reuses_its_controller_without_nesting() {
        let controller = ScrollController::new();
        let existing = Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 800.), Color::WHITE),
        );
        let widget = Scrollbar::new(existing).build(&ControlTheme::light());
        assert!(matches!(widget.kind, WidgetKind::Scroll { .. }));

        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount scrollbar");
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        assert!(tree.scroll_at(Offset::new(10., 50.), Offset::new(0., 100.)));
        assert!(controller.offset() > 0.);
    }
}
