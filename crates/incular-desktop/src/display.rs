use incular_platform::{
    CapabilitySupport, DisplayId, DisplaySnapshot, NativeWindowSystem, PhysicalScreenRect,
    PhysicalSize,
};
use winit::monitor::MonitorHandle;

use crate::display_identity::GenerationalDisplayRegistry;

/// Narrow native-service seam for desktop geometry Winit intentionally does
/// not expose, such as a monitor's taskbar/dock-adjusted work area.
///
/// OS facade crates implement this when they can make a reliable native query.
/// Returning `Unsupported`/`None` is preferable to deriving a fake usable area
/// from full monitor bounds.
pub trait DesktopPlatformServices {
    fn work_area_support(&self, _system: NativeWindowSystem) -> CapabilitySupport {
        CapabilitySupport::Unsupported
    }

    fn work_area(&self, _monitor: &MonitorHandle) -> Option<PhysicalScreenRect> {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultDesktopPlatformServices;

impl DesktopPlatformServices for DefaultDesktopPlatformServices {}

#[derive(Default)]
pub(crate) struct DesktopDisplayRegistry {
    identities: GenerationalDisplayRegistry<MonitorHandle>,
    last_snapshots: Vec<DisplaySnapshot>,
    last_primary: Option<DisplayId>,
}

pub(crate) struct DisplaySync {
    pub(crate) snapshots: Vec<DisplaySnapshot>,
    pub(crate) primary: Option<DisplayId>,
    pub(crate) changed: bool,
}

impl DesktopDisplayRegistry {
    pub(crate) fn synchronize(
        &mut self,
        monitors: Vec<MonitorHandle>,
        primary: Option<MonitorHandle>,
        system: NativeWindowSystem,
        services: &dyn DesktopPlatformServices,
    ) -> DisplaySync {
        self.identities.synchronize(&monitors);
        let primary = primary
            .as_ref()
            .and_then(|monitor| self.identities.id_for(monitor));
        let exposes_global_bounds = !matches!(system, NativeWindowSystem::Wayland);
        let mut snapshots = self
            .identities
            .connected()
            .map(|(id, monitor)| {
                let size = monitor.size();
                let physical_size = PhysicalSize::new(size.width, size.height);
                let physical_bounds = exposes_global_bounds.then(|| {
                    let position = monitor.position();
                    PhysicalScreenRect::new(
                        position.x,
                        position.y,
                        physical_size.width,
                        physical_size.height,
                    )
                });
                let physical_work_area = exposes_global_bounds
                    .then(|| services.work_area(monitor))
                    .flatten();
                DisplaySnapshot {
                    id,
                    name: monitor.name(),
                    scale_factor: monitor.scale_factor(),
                    physical_size,
                    physical_bounds,
                    // Dividing a desktop-global physical origin by this
                    // monitor's scale is wrong on a mixed-DPI desktop. Until a
                    // backend supplies a coherent logical global space, leave
                    // these explicitly unavailable.
                    logical_bounds: None,
                    physical_work_area,
                    logical_work_area: None,
                    is_primary: primary == Some(id),
                }
            })
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|display| display.id);
        let changed = snapshots != self.last_snapshots || primary != self.last_primary;
        self.last_snapshots = snapshots.clone();
        self.last_primary = primary;
        DisplaySync {
            snapshots,
            primary,
            changed,
        }
    }

    pub(crate) fn id_for(&self, monitor: &MonitorHandle) -> Option<DisplayId> {
        self.identities.id_for(monitor)
    }
}
