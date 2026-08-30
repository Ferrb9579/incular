//! Diagnostics, debugging, keep-alive, and framework utility widgets.

use crate::{Center, ColoredBox, Column, Text, Widget};
use incular_core::Color;
use typed_builder::TypedBuilder;

/// An expandable/collapsible container widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Expansible {
    #[builder(default = false)]
    expanded: bool,
    #[builder(setter(into))]
    header: Widget,
    #[builder(setter(into))]
    body: Widget,
}

impl Expansible {
    #[must_use]
    pub fn new(header: impl Into<Widget>, body: impl Into<Widget>) -> Self {
        Self {
            expanded: false,
            header: header.into(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }
}

impl From<Expansible> for Widget {
    fn from(value: Expansible) -> Self {
        if value.expanded {
            Column::new([value.header, value.body]).into()
        } else {
            value.header
        }
    }
}

/// Displays framework build or layout runtime errors cleanly on screen.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ErrorWidget {
    #[builder(default = String::new(), setter(into))]
    message: String,
}

impl Default for ErrorWidget {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ErrorWidget {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<ErrorWidget> for Widget {
    fn from(value: ErrorWidget) -> Self {
        ColoredBox::new(
            Color::rgba(180, 40, 40, 255),
            Center::new(Text::new(format!("Error: {}", value.message))),
        )
        .into()
    }
}

/// Mount point for the runtime performance overlay.
///
/// The widget itself is intentionally inert until the application installs a
/// runtime overlay with `Runtime::install_performance_overlay`. Keeping the
/// mount point as a real keyed retained node means installation updates one
/// element instead of rebuilding the application root.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PerformanceOverlay;

impl PerformanceOverlay {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl From<PerformanceOverlay> for Widget {
    fn from(_: PerformanceOverlay) -> Self {
        crate::tree::performance_overlay_placeholder()
    }
}
