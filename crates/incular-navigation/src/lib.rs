//! Stack navigation, deep-link routing, route transitions, and overlays.
//!
//! The subsystem depends one-way on widget descriptions and retained
//! transition layers. It owns no widget tree or renderer state.

mod navigator;
mod presentation;
mod registry;
mod route_data;

#[cfg(test)]
mod tests;

pub use navigator::{
    BackDispatchReport, BackDispatcher, NavigationEvent, Navigator, NavigatorObserver, PopDecision,
    PopResult,
};
pub use presentation::{
    BottomSheet, BottomSheetBuilder, Dialog, DialogBuilder, ModalBarrier, ModalBarrierBuilder,
    Overlay, OverlayEntry, OverlayEntryBuilder, Page, PageDescriptorBuilder, PageRouteBuilder,
    PageRouteDescriptorBuilder, Route, RouteBuilder, RoutePresentation, RouteResult,
    RouteTransition,
};
pub use registry::RouteRegistry;
pub use route_data::{
    NAVIGATOR_SNAPSHOT_FORMAT_VERSION, NavigationRestoreReport, NavigatorSnapshot,
    RestorableNavigationError, RestorableRoute, RestorableRouteBuildError, RestorableRouteId,
    RestorableRouteIdError, RestorableRouteRegistrationError, RouteId, RouteScopeKey,
    RouteScopeKeyError, RouteSettings,
};
