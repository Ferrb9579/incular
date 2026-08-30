use incular_widgets::{ScrollConfiguration, ScrollPhysics, Widget};
use typed_builder::TypedBuilder;

/// The input device classes accepted by the Material scroll behavior.
///
/// The retained gesture layer performs the actual dispatch.  Keeping this
/// policy as data lets platform adapters add/remove device classes without
/// coupling the Material crate to a native event enum.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MaterialPointerDevice {
    #[default]
    Touch,
    Mouse,
    Trackpad,
    Stylus,
    Unknown,
}

/// Flutter-shaped scroll behavior configuration for a Material application.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct MaterialScrollBehavior {
    #[builder(default = ScrollPhysics::clamping())]
    physics: ScrollPhysics,
    #[builder(default = true)]
    scrollbars: bool,
    #[builder(
        default = vec![
            MaterialPointerDevice::Touch,
            MaterialPointerDevice::Mouse,
            MaterialPointerDevice::Trackpad,
            MaterialPointerDevice::Stylus,
        ],
        setter(transform = |devices: impl IntoIterator<Item = MaterialPointerDevice>| {
            devices.into_iter().collect::<Vec<_>>()
        })
    )]
    drag_devices: Vec<MaterialPointerDevice>,
}

impl Default for MaterialScrollBehavior {
    fn default() -> Self {
        Self {
            physics: ScrollPhysics::clamping(),
            scrollbars: true,
            drag_devices: vec![
                MaterialPointerDevice::Touch,
                MaterialPointerDevice::Mouse,
                MaterialPointerDevice::Trackpad,
                MaterialPointerDevice::Stylus,
            ],
        }
    }
}

impl MaterialScrollBehavior {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = physics;
        self
    }

    #[must_use]
    pub fn scrollbars(mut self, enabled: bool) -> Self {
        self.scrollbars = enabled;
        self
    }

    #[must_use]
    pub fn drag_devices(
        mut self,
        devices: impl IntoIterator<Item = MaterialPointerDevice>,
    ) -> Self {
        self.drag_devices = devices.into_iter().collect();
        self
    }

    #[must_use]
    pub fn get_physics(&self) -> ScrollPhysics {
        self.physics
    }

    #[must_use]
    pub fn get_scrollbars(&self) -> bool {
        self.scrollbars
    }

    #[must_use]
    pub fn get_drag_devices(&self) -> &[MaterialPointerDevice] {
        &self.drag_devices
    }

    /// Wraps a subtree in the retained ambient scroll configuration.
    ///
    /// `scrollbars` and `drag_devices` remain policy data for the platform
    /// adapter; physics is the part consumed directly by retained scroll
    /// views today.
    #[must_use]
    pub fn wrap(&self, child: impl Into<Widget>) -> Widget {
        ScrollConfiguration::new(child).physics(self.physics).into()
    }
}
