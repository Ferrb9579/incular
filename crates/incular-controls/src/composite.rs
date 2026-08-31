//! Shared behavior and keyboard-navigation primitives used by compound
//! controls. These types intentionally do not paint anything.

use incular_config::Axis;
use incular_widgets::Widget;
use incular_widgets::internal::ExplicitSemantics;
use std::cell::Cell;
use std::rc::Rc;

/// A transparent behavior-composition wrapper.
///
/// A slot lets a trigger/label/indicator contribute behavior and semantics
/// while retaining the caller's visual widget. It is deliberately a no-op in
/// layout and paint, so nested slots do not introduce competing hit targets.
#[derive(Clone)]
pub struct Slot {
    child: Widget,
    semantics: Option<ExplicitSemantics>,
}

impl Slot {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            semantics: None,
        }
    }

    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    #[must_use]
    pub fn into_child(self) -> Widget {
        let Self { child, semantics } = self;
        semantics.map_or(child.clone(), |semantics| child.semantics(semantics))
    }

    /// Adds semantic behavior without adding a visual or layout wrapper.
    /// Multiple behavior wrappers can therefore decorate one custom visual
    /// while the retained tree still exposes a single hit target.
    #[must_use]
    pub fn semantics(mut self, value: ExplicitSemantics) -> Self {
        self.semantics = Some(value);
        self
    }
}

impl From<Slot> for Widget {
    fn from(value: Slot) -> Self {
        value.into_child()
    }
}

/// Orientation used by composite controls. This is separate from layout's
/// axis so keyboard navigation can be expressed without a render dependency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompositeOrientation {
    #[default]
    Horizontal,
    Vertical,
}

impl From<CompositeOrientation> for Axis {
    fn from(value: CompositeOrientation) -> Self {
        match value {
            CompositeOrientation::Horizontal => Axis::Horizontal,
            CompositeOrientation::Vertical => Axis::Vertical,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositeItem {
    pub id: String,
    pub disabled: bool,
}

/// Retained roving-focus/typeahead state shared by tabs, menus, toolbars,
/// radio groups, and select-like controls.
#[derive(Clone)]
pub struct CompositeController {
    items: Rc<Vec<CompositeItem>>,
    active: Rc<Cell<Option<usize>>>,
    orientation: CompositeOrientation,
    loop_navigation: bool,
    rtl: bool,
}

impl Default for CompositeController {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeController {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Rc::new(Vec::new()),
            active: Rc::new(Cell::new(None)),
            orientation: CompositeOrientation::Horizontal,
            loop_navigation: true,
            rtl: false,
        }
    }

    #[must_use]
    pub fn items(&self) -> &[CompositeItem] {
        self.items.as_slice()
    }

    pub fn set_items(&mut self, items: impl IntoIterator<Item = CompositeItem>) {
        self.items = Rc::new(items.into_iter().collect());
        self.active
            .set(self.active.get().filter(|index| *index < self.items.len()));
    }

    #[must_use]
    pub fn orientation(&self) -> CompositeOrientation {
        self.orientation
    }
    #[must_use]
    pub fn orientation_set(mut self, orientation: CompositeOrientation) -> Self {
        self.orientation = orientation;
        self
    }
    #[must_use]
    pub fn loop_navigation(mut self, value: bool) -> Self {
        self.loop_navigation = value;
        self
    }
    #[must_use]
    pub fn rtl(mut self, value: bool) -> Self {
        self.rtl = value;
        self
    }
    #[must_use]
    pub fn active(&self) -> Option<usize> {
        self.active.get()
    }
    pub fn set_active(&self, index: Option<usize>) {
        self.active.set(index.filter(|i| *i < self.items.len()));
    }

    fn seek(&self, start: Option<usize>, direction: isize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let direction = if self.rtl && self.orientation == CompositeOrientation::Horizontal {
            -direction
        } else {
            direction
        };
        if start.is_none() {
            return if direction >= 0 {
                self.first()
            } else {
                self.last()
            };
        }
        let origin = start.expect("checked above");
        let mut index = origin as isize;
        for _ in 0..self.items.len() {
            index += direction;
            if self.loop_navigation {
                index = index.rem_euclid(self.items.len() as isize);
            } else if index < 0 || index >= self.items.len() as isize {
                return None;
            }
            let candidate = index as usize;
            if !self.items[candidate].disabled {
                return Some(candidate);
            }
        }
        None
    }

    #[must_use]
    pub fn next(&self) -> Option<usize> {
        self.seek(self.active.get(), 1)
    }
    #[must_use]
    pub fn previous(&self) -> Option<usize> {
        self.seek(self.active.get(), -1)
    }
    #[must_use]
    pub fn first(&self) -> Option<usize> {
        self.items.iter().position(|item| !item.disabled)
    }
    #[must_use]
    pub fn last(&self) -> Option<usize> {
        self.items.iter().rposition(|item| !item.disabled)
    }

    /// Finds the first enabled item whose id starts with a typed prefix.
    /// Matching is case-insensitive; callers can retain the returned index
    /// and update it without rebuilding the composite widget.
    #[must_use]
    pub fn typeahead(&self, prefix: &str) -> Option<usize> {
        let prefix = prefix.trim().to_lowercase();
        if prefix.is_empty() {
            return self.active();
        }
        self.items.iter().enumerate().find_map(|(index, item)| {
            (!item.disabled && item.id.to_lowercase().starts_with(&prefix)).then_some(index)
        })
    }
}

/// Returns a child from a transparent scope without accidentally introducing
/// a nested visual wrapper. Used by advanced components that accept either a
/// plain widget or a `Slot`.
#[must_use]
pub fn slot_child(widget: Widget) -> Widget {
    widget
}
