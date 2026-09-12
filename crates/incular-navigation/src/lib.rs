//! Stack navigation, deep-link routing, route transitions, and overlays.
//!
//! The subsystem depends one-way on widget descriptions and retained
//! transition layers. It owns no widget tree or renderer state.

mod navigator;
mod presentation;
mod registry;
mod route_data;

pub use navigator::{
    BackAttachError, BackDispatchReport, BackDispatcher, DuplicatePageKey, NavigationEvent,
    Navigator, NavigatorObserver, PopDecision, PopResult, RouteLifetime, RouteLifetimeSubscription,
};
pub use presentation::{
    BottomSheet, BottomSheetBuilder, Dialog, DialogBuilder, ModalBarrier, ModalBarrierBuilder,
    Overlay, OverlayEntry, OverlayEntryBuilder, Page, PageDescriptorBuilder, PageRouteBuilder,
    PageRouteDescriptorBuilder, Route, RouteBuilder, RoutePresentation, RouteResult,
    RouteTransition,
};
pub use registry::RouteRegistry;
pub use route_data::{
    NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigationRestoreReport, NavigatorSnapshot, PageKey,
    PageKeyError, RestorableNavigationError, RestorableRoute, RestorableRouteBuildError,
    RestorableRouteId, RestorableRouteIdError, RestorableRouteRegistrationError, RouteId,
    RouteScopeKey, RouteScopeKeyError, RouteSettings,
};
