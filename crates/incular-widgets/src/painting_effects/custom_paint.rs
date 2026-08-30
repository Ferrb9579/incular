//! Renderer-independent custom painting contract.

/// A delegate that produces custom display-list drawing commands.
pub trait CustomPainter {
    /// Draws visual content into `canvas` bounds.
    fn paint(&self, size: incular_core::Size) -> incular_rendering::DisplayList;
    /// Decides whether a repaint is required when compared to a previous painter instance.
    fn should_repaint(&self, old: &Self) -> bool
    where
        Self: Sized;
}
