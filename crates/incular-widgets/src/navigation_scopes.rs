//! Declarative navigation, restoration scopes, application roots, and multi-view widgets.

use crate::{
    Positioned, TransientDismissPolicy, TransientDismissReason, TransientPlacement,
    TransientPresentation, TransientRole, Widget,
    transient::{TransientPlacementOverride, TransientPortalMarker},
};
use std::rc::Rc;

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
    placement: Option<TransientPlacement>,
    anchor_override: Option<incular_core::Rect>,
    dismiss_policy: Option<TransientDismissPolicy>,
    on_dismiss: Option<Rc<dyn Fn(TransientDismissReason) + 'static>>,
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
            placement: None,
            anchor_override: None,
            dismiss_policy: None,
            on_dismiss: None,
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
    pub fn placement(mut self, placement: TransientPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    /// Overrides the retained anchor geometry with a view-local logical rect.
    /// Context menus use a zero-size rect at the pointer position.
    #[must_use]
    pub fn anchor_rect(mut self, anchor: incular_core::Rect) -> Self {
        self.anchor_override = Some(anchor);
        self
    }

    #[must_use]
    pub fn anchor_point(mut self, point: incular_core::Offset) -> Self {
        self.anchor_override = Some(incular_core::Rect::from_origin_size(
            point,
            incular_core::Size::ZERO,
        ));
        self
    }

    #[must_use]
    pub fn dismiss_policy(mut self, policy: TransientDismissPolicy) -> Self {
        self.dismiss_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn on_dismiss(mut self, callback: impl Fn(TransientDismissReason) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(callback));
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
        let popup = value
            .overlay_child
            .clone()
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        let positioner = popup
            .environment_value::<TransientPlacementOverride>()
            .copied();
        let placement = value
            .placement
            .or_else(|| positioner.map(|override_| override_.placement))
            .unwrap_or_default();
        let anchor_override = value
            .anchor_override
            .or_else(|| positioner.and_then(|override_| override_.anchor_override));
        let dismiss_policy = value
            .dismiss_policy
            .unwrap_or_else(|| TransientDismissPolicy::for_role(value.role));
        let on_dismiss = value.on_dismiss.clone();
        let marker;
        let child = if value.show_overlay {
            // A transient never participates in the owning stack's intrinsic
            // size. The retained placement pass measures this positioned child
            // independently and assigns its final offset after root layout.
            let popup: Widget = Positioned::new(popup).into();
            let mut children = Vec::with_capacity(3);
            if let Some(barrier) = value.barrier_child {
                children.push(barrier);
            }
            // The barrier belongs behind the anchor. Besides matching paint
            // order, this lets an open menu's own anchor remain interactive so
            // clicking it can toggle the menu closed instead of being consumed
            // by the full-view dismissal layer.
            let anchor_child_index = children.len();
            children.push(value.child);
            let popup_child_index = children.len();
            children.push(popup);
            marker = TransientPortalMarker {
                role: value.role,
                presentation: value.presentation,
                placement,
                anchor_override,
                dismiss_policy,
                on_dismiss: on_dismiss.clone(),
                show: true,
                anchor_child_index,
                popup_child_index,
            };
            let stack: Widget = crate::Stack::new(children)
                .clip_behavior(incular_config::Clip::None)
                .into();
            stack.semantics(crate::internal::ExplicitSemantics::new(
                incular_semantics::SemanticRole::GenericContainer,
            ))
        } else {
            marker = TransientPortalMarker {
                role: value.role,
                presentation: value.presentation,
                placement,
                anchor_override,
                dismiss_policy,
                on_dismiss,
                show: false,
                anchor_child_index: 0,
                popup_child_index: 0,
            };
            value.child
        };
        Widget::environment_scope(marker, child)
    }
}
