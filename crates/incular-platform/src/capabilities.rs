//! Runtime platform capability snapshots.
//!
//! Capabilities describe the operations the active backend/session can
//! actually honor. They are intentionally grouped by domain rather than
//! flattened into one global bitset: a Wayland session, for example, can have
//! excellent window/input support while deliberately lacking global placement.

/// Whether a portable platform capability is known to be available.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CapabilitySupport {
    /// No native backend has published a capability snapshot yet.
    #[default]
    Unknown,
    /// The active backend/session can honor the operation.
    Supported,
    /// Incular or the active backend/session cannot honor the operation.
    Unsupported,
}

impl CapabilitySupport {
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    #[must_use]
    pub const fn from_supported(supported: bool) -> Self {
        if supported {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }
}

/// Portable operations performed on an already-created native window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WindowControlCapabilities {
    pub set_title: CapabilitySupport,
    pub set_visibility: CapabilitySupport,
    pub set_logical_size: CapabilitySupport,
    pub begin_move_drag: CapabilitySupport,
    pub begin_resize_drag: CapabilitySupport,
    pub minimize: CapabilitySupport,
    pub maximize: CapabilitySupport,
    pub fullscreen: CapabilitySupport,
    pub set_resizable: CapabilitySupport,
    pub set_decorations: CapabilitySupport,
    pub set_size_limits: CapabilitySupport,
    pub set_window_level: CapabilitySupport,
    pub set_window_icon: CapabilitySupport,
    pub request_user_attention: CapabilitySupport,
    pub content_sensitivity: CapabilitySupport,
    pub request_focus: CapabilitySupport,
    pub request_redraw: CapabilitySupport,
    pub close: CapabilitySupport,
}

/// Display discovery and top-level placement capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DisplayPlacementCapabilities {
    pub enumerate_displays: CapabilitySupport,
    pub query_current_display: CapabilitySupport,
    pub display_bounds: CapabilitySupport,
    pub work_area: CapabilitySupport,
    pub query_window_position: CapabilitySupport,
    pub set_window_position: CapabilitySupport,
}

/// Native surface hosting for transient UI such as menus and popovers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TransientSurfaceCapabilities {
    pub native_surface: CapabilitySupport,
}

/// Operating-system application-menu integration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NativeMenuCapabilities {
    pub application_menu: CapabilitySupport,
}

/// Data exchange with the operating system and other applications.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DataTransferCapabilities {
    pub clipboard_text: CapabilitySupport,
    pub clipboard_rich: CapabilitySupport,
    pub external_drag_drop: CapabilitySupport,
}

/// Application-level desktop services that live outside ordinary widgets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ApplicationServiceCapabilities {
    pub file_dialogs: CapabilitySupport,
    pub global_shortcuts: CapabilitySupport,
    pub single_instance_activation: CapabilitySupport,
    pub tray_or_status_item: CapabilitySupport,
    pub notifications: CapabilitySupport,
}

/// Device-rich input and native cursor facilities beyond basic pointer input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AdvancedInputCapabilities {
    pub pointer_metadata: CapabilitySupport,
    pub cursor_control: CapabilitySupport,
    pub stylus: CapabilitySupport,
    pub trackpad_gestures: CapabilitySupport,
}

/// Complete capability snapshot published by a platform backend.
///
/// `Default` means "not discovered yet", not unsupported. Headless/custom
/// embedders that intentionally provide no native services should publish
/// [`Self::unsupported`] so callers can distinguish those states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PlatformCapabilities {
    pub window: WindowControlCapabilities,
    pub display: DisplayPlacementCapabilities,
    pub transients: TransientSurfaceCapabilities,
    pub menus: NativeMenuCapabilities,
    pub data_transfer: DataTransferCapabilities,
    pub application_services: ApplicationServiceCapabilities,
    pub advanced_input: AdvancedInputCapabilities,
}

impl PlatformCapabilities {
    /// A deterministic capability snapshot for a backend that intentionally
    /// implements none of the native facilities represented here.
    #[must_use]
    pub const fn unsupported() -> Self {
        let unsupported = CapabilitySupport::Unsupported;
        Self {
            window: WindowControlCapabilities {
                set_title: unsupported,
                set_visibility: unsupported,
                set_logical_size: unsupported,
                begin_move_drag: unsupported,
                begin_resize_drag: unsupported,
                minimize: unsupported,
                maximize: unsupported,
                fullscreen: unsupported,
                set_resizable: unsupported,
                set_decorations: unsupported,
                set_size_limits: unsupported,
                set_window_level: unsupported,
                set_window_icon: unsupported,
                request_user_attention: unsupported,
                content_sensitivity: unsupported,
                request_focus: unsupported,
                request_redraw: unsupported,
                close: unsupported,
            },
            display: DisplayPlacementCapabilities {
                enumerate_displays: unsupported,
                query_current_display: unsupported,
                display_bounds: unsupported,
                work_area: unsupported,
                query_window_position: unsupported,
                set_window_position: unsupported,
            },
            transients: TransientSurfaceCapabilities {
                native_surface: unsupported,
            },
            menus: NativeMenuCapabilities {
                application_menu: unsupported,
            },
            data_transfer: DataTransferCapabilities {
                clipboard_text: unsupported,
                clipboard_rich: unsupported,
                external_drag_drop: unsupported,
            },
            application_services: ApplicationServiceCapabilities {
                file_dialogs: unsupported,
                global_shortcuts: unsupported,
                single_instance_activation: unsupported,
                tray_or_status_item: unsupported,
                notifications: unsupported,
            },
            advanced_input: AdvancedInputCapabilities {
                pointer_metadata: unsupported,
                cursor_control: unsupported,
                stylus: unsupported,
                trackpad_gestures: unsupported,
            },
        }
    }
}
