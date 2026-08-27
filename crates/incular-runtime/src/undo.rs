//! Retained undo/redo state for editable text.
//!
//! The controller owns history independently from any widget element. It
//! observes the authoritative `TextEditingController`, stores complete
//! editing values (including selection), and exposes availability through the
//! runtime's `Signal` type so only consumers that read history state rebuild.

use crate::Signal;
use incular_text::{TextEditingController, TextEditingValue};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Observable availability state for an [`UndoHistoryController`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UndoHistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
}

struct History {
    current: TextEditingValue,
    undo: Vec<TextEditingValue>,
    redo: Vec<TextEditingValue>,
}

struct Inner {
    editor: TextEditingController,
    history: RefCell<History>,
    applying: Cell<bool>,
    state: Signal<UndoHistoryState>,
    listener: usize,
}

impl Inner {
    fn availability(&self) -> UndoHistoryState {
        let history = self.history.borrow();
        UndoHistoryState {
            can_undo: !history.undo.is_empty(),
            can_redo: !history.redo.is_empty(),
        }
    }

    fn sync_state(&self) {
        let _ = self.state.set(self.availability());
    }

    fn record(&self, value: TextEditingValue) {
        if self.applying.get() {
            self.history.borrow_mut().current = value;
            return;
        }

        let changed = {
            let mut history = self.history.borrow_mut();
            if history.current == value {
                false
            } else {
                let previous = std::mem::replace(&mut history.current, value);
                history.undo.push(previous);
                history.redo.clear();
                true
            }
        };
        if changed {
            self.sync_state();
        }
    }

    fn apply_history_value(&self, value: TextEditingValue) {
        self.applying.set(true);
        self.editor.set_value(value);
        self.applying.set(false);
        self.sync_state();
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.editor.remove_listener(self.listener);
    }
}

/// A cloneable, widget-independent undo/redo controller for text editing.
#[derive(Clone)]
pub struct UndoHistoryController {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for UndoHistoryController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UndoHistoryController")
            .field("editor", &self.inner.editor)
            .field("state", &self.status())
            .finish()
    }
}

impl UndoHistoryController {
    /// Starts tracking an existing editor without adding its initial value to
    /// history.
    #[must_use]
    pub fn new(editor: TextEditingController) -> Self {
        let initial = editor.value();
        let inner = Rc::new_cyclic(|weak: &Weak<Inner>| {
            let listener = {
                let weak = weak.clone();
                editor.add_listener(move |value| {
                    if let Some(inner) = weak.upgrade() {
                        inner.record(value.clone());
                    }
                })
            };
            Inner {
                editor: editor.clone(),
                history: RefCell::new(History {
                    current: initial.clone(),
                    undo: Vec::new(),
                    redo: Vec::new(),
                }),
                applying: Cell::new(false),
                state: Signal::new(UndoHistoryState::default()),
                listener,
            }
        });
        Self { inner }
    }

    #[must_use]
    pub fn editor(&self) -> TextEditingController {
        self.inner.editor.clone()
    }

    /// Returns the signal used by reactive consumers to observe availability.
    #[must_use]
    pub fn state_signal(&self) -> Signal<UndoHistoryState> {
        self.inner.state.clone()
    }

    /// Short alias for [`Self::state_signal`].
    #[must_use]
    pub fn signal(&self) -> Signal<UndoHistoryState> {
        self.state_signal()
    }

    #[must_use]
    pub fn status(&self) -> UndoHistoryState {
        self.inner.state.get()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.status().can_undo
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.status().can_redo
    }

    /// Moves the editor to the previous value, if one exists.
    #[must_use]
    pub fn undo(&self) -> bool {
        let target = {
            let mut history = self.inner.history.borrow_mut();
            let Some(target) = history.undo.pop() else {
                return false;
            };
            let current = std::mem::replace(&mut history.current, target.clone());
            history.redo.push(current);
            target
        };
        self.inner.apply_history_value(target);
        true
    }

    /// Moves the editor to the next value, if one exists.
    #[must_use]
    pub fn redo(&self) -> bool {
        let target = {
            let mut history = self.inner.history.borrow_mut();
            let Some(target) = history.redo.pop() else {
                return false;
            };
            let current = std::mem::replace(&mut history.current, target.clone());
            history.undo.push(current);
            target
        };
        self.inner.apply_history_value(target);
        true
    }

    /// Discards undo and redo entries while retaining the current editor text.
    pub fn clear(&self) {
        let mut history = self.inner.history.borrow_mut();
        history.current = self.inner.editor.value();
        history.undo.clear();
        history.redo.clear();
        drop(history);
        self.inner.sync_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_controller_undo_redo() {
        let editor = TextEditingController::with_text("one");
        let controller = UndoHistoryController::new(editor.clone());
        editor.set_text("two");
        editor.set_text("three");
        assert!(controller.can_undo());
        assert!(!controller.can_redo());
        assert!(controller.undo());
        assert_eq!(editor.text(), "two");
        assert!(controller.undo());
        assert_eq!(editor.text(), "one");
        assert!(!controller.undo());
        assert!(controller.redo());
        assert_eq!(editor.text(), "two");
        assert!(controller.redo());
        assert_eq!(editor.text(), "three");
        assert!(!controller.can_redo());
    }

    #[test]
    fn undo_controller_uses_signal_state() {
        let editor = TextEditingController::new();
        let controller = UndoHistoryController::new(editor.clone());
        let state = controller.state_signal();
        assert_eq!(state.get(), UndoHistoryState::default());
        editor.set_text("changed");
        assert_eq!(
            state.get(),
            UndoHistoryState {
                can_undo: true,
                can_redo: false
            }
        );
        assert!(controller.undo());
        assert_eq!(
            state.get(),
            UndoHistoryState {
                can_undo: false,
                can_redo: true
            }
        );
        controller.clear();
        assert_eq!(state.get(), UndoHistoryState::default());
    }
}
