//! Declarative navigation, restoration scopes, application roots, and multi-view widgets.

use crate::Widget;

/// An overlay portal that renders its overlay child in an ancestor `Overlay`.
#[derive(Clone)]
pub struct OverlayPortal {
    show_overlay: bool,
    overlay_child: Option<Widget>,
    child: Widget,
}

impl OverlayPortal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            show_overlay: false,
            overlay_child: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn overlay_child(mut self, overlay: impl Into<Widget>) -> Self {
        self.overlay_child = Some(overlay.into());
        self
    }

    #[must_use]
    pub fn show(mut self, show: bool) -> Self {
        self.show_overlay = show;
        self
    }
}

impl From<OverlayPortal> for Widget {
    fn from(value: OverlayPortal) -> Self {
        if value.show_overlay {
            // A window root may not have an explicit overlay host yet. Keep
            // the retained logical owner and present the overlay above it in
            // a deterministic stack; native window adapters can lift this
            // stack into their overlay layer without changing the control
            // descriptor API.
            crate::Stack::new([
                value.child,
                value
                    .overlay_child
                    .unwrap_or_else(|| crate::SizedBox::shrink().into()),
            ])
            .into()
        } else {
            value.child
        }
    }
}
