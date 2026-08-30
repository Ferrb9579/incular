//! Application and multi-view shell widgets.

use crate::{
    CheckedModeBanner, Directionality, ErrorWidget, Localizations, MediaQuery, SizedBox, Widget,
};

mod app;
mod router;
mod title;
mod view;

pub use app::{
    ApplicationBootstrapHost, ApplicationBootstrapOptions, ApplicationBootstrapSpec, WidgetsApp,
    WidgetsAppController, WidgetsAppData,
};
pub use router::{
    BackButtonDispatcher, BackCallbackSubscription, BasicRouterDelegate,
    ClosureRouteInformationParser, MemoryRouteInformationProvider, NavigationNotification,
    NavigationNotificationKind, NavigationNotificationListener, NavigationNotificationSubscription,
    RootBackButtonDispatcher, RouteInformation, RouteInformationListener, RouteInformationParser,
    RouteInformationProvider, RouteInformationReportingType, RouteInformationSubscription, Router,
    RouterConfig, RouterData, RouterDelegate, RouterDelegateListener, RouterDelegateSubscription,
    RouterError, StringRouteInformationParser, normalize_route_location,
};
pub use title::{
    NoopWindowChromeSink, Title, TitleController, TitleData, TitleError, WindowChromeSink,
};
pub use view::{
    AuxiliaryViewError, AuxiliaryViewHandle, AuxiliaryViewHost, AuxiliaryViewOutcome,
    AuxiliaryViewRequest, NoopAuxiliaryViewHost, View, ViewAnchor, ViewAnchorController,
    ViewAnchorData, ViewAnchorSubscription, ViewController, ViewData, ViewEvent, ViewId,
    ViewLifecycle, ViewMetrics, ViewSubscription,
};
