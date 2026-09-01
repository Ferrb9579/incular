//! Declarative navigation, restoration scopes, application roots, and multi-view widgets.

use crate::{TransientPresentation, TransientRole, Widget, transient::TransientPortalMarker};

#[allow(unused_imports)]
pub use crate::navigation::{
    AnimatedModalBarrier, BackButtonDispatcher, BackButtonListener, BackDispatchReport,
    BackHandlerResult, BackRegistration, NavigatorPopHandler, NavigatorPopHandlerController,
    PageStorage, PageStorageBucket, PageStorageIdentifier, PageStorageKey, PopAttempt, PopScope,
    PopScopeController, RootRestorationScope, UnmanagedRestorationScope,
    current_page_storage_bucket, current_restoration_scope,
};

/// A semantic portal for transient content.
///
/// The retained portal keeps its anchor and transient content associated even
/// when a desktop backend lifts the transient into a separate native popup
/// surface. The current stable desktop backend can preserve the same contract
/// with an in-view overlay until native popup roles are available.
#[derive(Clone)]
pub struct OverlayPortal {
    show_overlay: bool,
    overlay_child: Option<Widget>,
    barrier_child: Option<Widget>,
    presentation: TransientPresentation,
    role: TransientRole,
    child: Widget,
}

impl OverlayPortal {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            show_overlay: false,
            overlay_child: None,
            barrier_child: None,
            presentation: TransientPresentation::Auto,
            role: TransientRole::Popover,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn overlay_child(mut self, overlay: impl Into<Widget>) -> Self {
        self.overlay_child = Some(overlay.into());
        self
    }

    /// Adds an owning-view barrier used by overlay presentation for outside
    /// activation/dismissal. It is deliberately separate from popup content so
    /// a native surface backend never mistakes the full-window barrier for the
    /// popup's geometry.
    #[must_use]
    pub fn barrier_child(mut self, barrier: impl Into<Widget>) -> Self {
        self.barrier_child = Some(barrier.into());
        self
    }

    #[must_use]
    pub fn presentation(mut self, presentation: TransientPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    #[must_use]
    pub fn role(mut self, role: TransientRole) -> Self {
        self.role = role;
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
        let marker;
        let child = if value.show_overlay {
            let popup = value
                .overlay_child
                .unwrap_or_else(|| crate::SizedBox::shrink().into());
            let mut children = Vec::with_capacity(3);
            children.push(value.child);
            if let Some(barrier) = value.barrier_child {
                children.push(barrier);
            }
            let popup_child_index = children.len();
            children.push(popup);
            marker = TransientPortalMarker {
                role: value.role,
                presentation: value.presentation,
                show: true,
                popup_child_index,
            };
            crate::Stack::new(children)
                .clip_behavior(incular_config::Clip::None)
                .into()
        } else {
            marker = TransientPortalMarker {
                role: value.role,
                presentation: value.presentation,
                show: false,
                popup_child_index: 0,
            };
            value.child
        };
        Widget::environment_scope(marker, child)
    }
}
