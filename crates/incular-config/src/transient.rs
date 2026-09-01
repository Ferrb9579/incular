//! Cross-layer policy for short-lived UI presentation.

/// Preferred host for transient content such as menus and popovers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransientPresentation {
    /// Prefer a true transient surface when the platform backend supports the
    /// required role, otherwise preserve semantics with an in-view overlay.
    #[default]
    Auto,
    /// Always render inside the owning view.
    Overlay,
}

/// Semantic role of a short-lived anchored surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransientRole {
    #[default]
    Popover,
    Menu,
    ContextMenu,
    ComboBox,
    Tooltip,
}
