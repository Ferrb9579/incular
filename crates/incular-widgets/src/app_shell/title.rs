//! Retained application title and window-switcher metadata.

use super::Widget;
use incular_core::Color;
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

/// Title metadata made available to descendants and platform bridges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleData {
    pub title: String,
    pub color: Color,
}

/// Error returned when Flutter-compatible opaque title color is violated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleError {
    TransparentColor,
}

impl fmt::Display for TitleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransparentColor => formatter.write_str("application title color must be opaque"),
        }
    }
}

impl std::error::Error for TitleError {}

/// Runtime/platform bridge for retained title metadata.
pub trait WindowChromeSink: 'static {
    /// Applies the native window title.  `false` means the backend has no
    /// title capability or rejected the update.
    fn set_title(&self, title: &str) -> bool;

    /// Applies the application-switcher color when supported.  The default is
    /// a deterministic no-op for platforms without this concept.
    fn set_application_color(&self, _color: Color) -> bool {
        false
    }
}

/// Explicit no-op chrome sink for headless or unsupported platforms.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopWindowChromeSink;

impl WindowChromeSink for NoopWindowChromeSink {
    fn set_title(&self, _title: &str) -> bool {
        false
    }
}

struct TitleState {
    data: TitleData,
    sink: Option<Rc<dyn WindowChromeSink>>,
    last_sent_title: Option<String>,
    last_sent_color: Option<Color>,
    revision: Rc<Cell<u64>>,
}

/// Retained title owner shared by `Title` rebuilds and a native window bridge.
#[derive(Clone)]
pub struct TitleController {
    state: Rc<RefCell<TitleState>>,
}

impl fmt::Debug for TitleController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TitleController")
            .field("data", &self.data())
            .field("revision", &self.revision())
            .finish()
    }
}

impl TitleController {
    /// Creates a title controller with an opaque color.
    pub fn try_new(title: impl Into<String>, color: Color) -> Result<Self, TitleError> {
        if color.alpha != u8::MAX {
            return Err(TitleError::TransparentColor);
        }
        Ok(Self {
            state: Rc::new(RefCell::new(TitleState {
                data: TitleData {
                    title: title.into(),
                    color,
                },
                sink: None,
                last_sent_title: None,
                last_sent_color: None,
                revision: Rc::new(Cell::new(0)),
            })),
        })
    }

    /// Creates a title controller and panics on an invalid transparent color,
    /// matching Flutter's debug assertion for `Title.color`.
    #[must_use]
    pub fn new(title: impl Into<String>, color: Color) -> Self {
        Self::try_new(title, color).expect("Title color must be opaque")
    }

    #[must_use]
    pub fn data(&self) -> TitleData {
        self.state.borrow().data.clone()
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.state.borrow().data.title.clone()
    }

    #[must_use]
    pub fn color(&self) -> Color {
        self.state.borrow().data.color
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision.get()
    }

    /// Binds/rebinds a platform sink and immediately applies retained values.
    pub fn bind_sink(&self, sink: Rc<dyn WindowChromeSink>) {
        {
            let mut state = self.state.borrow_mut();
            state.sink = Some(sink);
            state.last_sent_title = None;
            state.last_sent_color = None;
        }
        self.synchronize();
    }

    /// Removes the platform sink while retaining title state.
    pub fn unbind_sink(&self) {
        let mut state = self.state.borrow_mut();
        state.sink = None;
        state.last_sent_title = None;
        state.last_sent_color = None;
    }

    /// Retains a new title and applies it once to the bound sink.
    pub fn set_title(&self, title: impl Into<String>) -> bool {
        let title = title.into();
        {
            let mut state = self.state.borrow_mut();
            if state.data.title == title {
                return false;
            }
            state.data.title = title;
            state.last_sent_title = None;
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        self.synchronize();
        true
    }

    /// Retains a new opaque switcher color and applies it when supported.
    pub fn set_color(&self, color: Color) -> Result<bool, TitleError> {
        if color.alpha != u8::MAX {
            return Err(TitleError::TransparentColor);
        }
        {
            let mut state = self.state.borrow_mut();
            if state.data.color == color {
                return Ok(false);
            }
            state.data.color = color;
            state.last_sent_color = None;
            state.revision.set(state.revision.get().wrapping_add(1));
        }
        self.synchronize();
        Ok(true)
    }

    /// Updates both retained fields atomically before synchronizing.
    pub fn set(&self, title: impl Into<String>, color: Color) -> Result<bool, TitleError> {
        if color.alpha != u8::MAX {
            return Err(TitleError::TransparentColor);
        }
        let title = title.into();
        let changed = {
            let mut state = self.state.borrow_mut();
            let changed = state.data.title != title || state.data.color != color;
            if changed {
                state.data = TitleData { title, color };
                state.last_sent_title = None;
                state.last_sent_color = None;
                state.revision.set(state.revision.get().wrapping_add(1));
            }
            changed
        };
        if changed {
            self.synchronize();
        }
        Ok(changed)
    }

    /// Applies only retained values not already accepted by the current sink.
    pub fn synchronize(&self) {
        let (sink, data, title_needed, color_needed) = {
            let state = self.state.borrow();
            let Some(sink) = state.sink.clone() else {
                return;
            };
            (
                sink,
                state.data.clone(),
                state.last_sent_title.as_deref() != Some(state.data.title.as_str()),
                state.last_sent_color != Some(state.data.color),
            )
        };
        let title_applied = !title_needed || sink.set_title(&data.title);
        let color_applied = !color_needed || sink.set_application_color(data.color);
        let mut state = self.state.borrow_mut();
        if title_needed && title_applied {
            state.last_sent_title = Some(data.title);
        }
        if color_needed && color_applied {
            state.last_sent_color = Some(data.color);
        }
    }
}

/// A retained title wrapper analogous to Flutter's `Title` widget.
#[derive(Clone)]
pub struct Title {
    controller: TitleController,
    child: Widget,
    sink: Option<Rc<dyn WindowChromeSink>>,
}

impl fmt::Debug for Title {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Title")
            .field("controller", &self.controller)
            .field("child", &self.child)
            .field("has_sink", &self.sink.is_some())
            .finish()
    }
}

impl Title {
    /// Creates a title wrapper with an opaque native switcher color.
    #[must_use]
    pub fn new(title: impl Into<String>, color: Color, child: impl Into<Widget>) -> Self {
        Self {
            controller: TitleController::new(title, color),
            child: child.into(),
            sink: None,
        }
    }

    /// Creates a title wrapper using the conventional opaque black metadata.
    #[must_use]
    pub fn simple(title: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self::new(title, Color::BLACK, child)
    }

    /// Fallible constructor for application configuration paths.
    pub fn try_new(
        title: impl Into<String>,
        color: Color,
        child: impl Into<Widget>,
    ) -> Result<Self, TitleError> {
        Ok(Self {
            controller: TitleController::try_new(title, color)?,
            child: child.into(),
            sink: None,
        })
    }

    /// Uses a controller whose values can be changed after widget creation.
    #[must_use]
    pub fn with_controller(mut self, controller: TitleController) -> Self {
        self.controller = controller;
        self
    }

    /// Binds a native chrome sink for the retained title.
    #[must_use]
    pub fn with_sink(mut self, sink: Rc<dyn WindowChromeSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    #[must_use]
    pub fn controller(&self) -> TitleController {
        self.controller.clone()
    }

    #[must_use]
    pub fn data(&self) -> TitleData {
        self.controller.data()
    }

    /// Builds a transparent retained scope while keeping native updates live.
    #[must_use]
    pub fn into_widget(self) -> Widget {
        let controller = self.controller;
        if let Some(sink) = self.sink {
            controller.bind_sink(sink);
        } else {
            controller.synchronize();
        }
        let child = self.child;
        let revision = controller.state.borrow().revision.clone();
        let retained_controller = controller.clone();
        Widget::stateful_layout_builder(revision, move |_, _| {
            retained_controller.synchronize();
            Widget::environment_scope(retained_controller.data(), child.clone())
        })
    }

    /// Alias for [`Self::into_widget`].
    #[must_use]
    pub fn widget(self) -> Widget {
        self.into_widget()
    }
}

impl From<Title> for Widget {
    fn from(value: Title) -> Self {
        value.into_widget()
    }
}
