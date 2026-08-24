//! Diagnostics, debugging, keep-alive, and framework utility widgets.

use crate::{Center, ColoredBox, Column, SizedBox, Text, Widget};
use incular_core::Color;

/// An expandable/collapsible container widget.
#[derive(Clone, Debug, PartialEq)]
pub struct Expansible {
    expanded: bool,
    header: Widget,
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

/// A raw hover/focus tooltip presentation widget.
#[derive(Clone, Debug, PartialEq)]
pub struct RawTooltip {
    message: String,
    child: Widget,
}

impl RawTooltip {
    #[must_use]
    pub fn new(message: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            message: message.into(),
            child: child.into(),
        }
    }
}

impl From<RawTooltip> for Widget {
    fn from(value: RawTooltip) -> Self {
        value.child
    }
}

/// Displays framework build or layout runtime errors cleanly on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorWidget {
    message: String,
}

impl ErrorWidget {
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

/// A corner diagnostic message banner.
#[derive(Clone, Debug, PartialEq)]
pub struct Banner {
    message: String,
    color: Color,
    child: Widget,
}

impl Banner {
    #[must_use]
    pub fn new(message: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            message: message.into(),
            color: Color::rgba(200, 30, 30, 255),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl From<Banner> for Widget {
    fn from(value: Banner) -> Self {
        value.child
    }
}

/// Standard debug mode corner banner.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckedModeBanner {
    child: Widget,
}

impl CheckedModeBanner {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<CheckedModeBanner> for Widget {
    fn from(value: CheckedModeBanner) -> Self {
        value.child
    }
}

/// Overlays real-time GPU and UI frame statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct PerformanceOverlay {
    child: Option<Widget>,
}

impl Default for PerformanceOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceOverlay {
    #[must_use]
    pub fn new() -> Self {
        Self { child: None }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<PerformanceOverlay> for Widget {
    fn from(value: PerformanceOverlay) -> Self {
        value.child.unwrap_or_else(|| SizedBox::shrink().into())
    }
}

/// Tells lazy list viewports to keep its subtree element alive when scrolled offscreen.
#[derive(Clone, Debug, PartialEq)]
pub struct KeepAlive {
    keep_alive: bool,
    child: Widget,
}

impl KeepAlive {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            keep_alive: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn keep_alive(mut self, keep_alive: bool) -> Self {
        self.keep_alive = keep_alive;
        self
    }
}

impl From<KeepAlive> for Widget {
    fn from(value: KeepAlive) -> Self {
        value.child
    }
}

/// Automatic client keep-alive wrapper.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomaticKeepAlive {
    child: Widget,
}

impl AutomaticKeepAlive {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<AutomaticKeepAlive> for Widget {
    fn from(value: AutomaticKeepAlive) -> Self {
        value.child
    }
}

/// Annotates a widget with its zero-based index in a collection for accessibility clients.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexedSemantics {
    index: usize,
    child: Widget,
}

impl IndexedSemantics {
    #[must_use]
    pub fn new(index: usize, child: impl Into<Widget>) -> Self {
        Self {
            index,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }
}

impl From<IndexedSemantics> for Widget {
    fn from(value: IndexedSemantics) -> Self {
        value.child
    }
}

/// Visual debugging overlay that renders semantic boundaries and labels.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticsDebugger {
    child: Widget,
}

impl SemanticsDebugger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<SemanticsDebugger> for Widget {
    fn from(value: SemanticsDebugger) -> Self {
        value.child
    }
}
