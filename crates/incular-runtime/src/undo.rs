//! Retained undo/redo state for editable text.
//!
//! The controller owns history independently from any widget element. It
//! observes the authoritative `TextEditingController`, stores complete
//! editing values (including selection), and exposes availability through the
//! runtime's `Signal` type so only consumers that read history state rebuild.
//!
//! History policy — what enters the stacks:
//! Undoable: observed committed text transitions, including typing,
//! deletion, cut/paste, programmatic sets and committed IME composition.
//! Historical notification arguments are not authoritative current state;
//! reentrant edits before this observer runs can coalesce intermediate values.
//! Transient, never a step: preedit set/clear, selection and caret moves,
//! and the composing marker. A cancelled composition leaves no trace, and
//! any committed value change clears the transient overlay so preedit never
//! describes a stale value. Material installs Widgets formatters through
//! retained subscriptions; Text applies them before committing or notifying
//! history, so rejected intermediate values are not recorded.
//! Redo clears on the next committed text change, including after an undo.
//! Controller replacement starts fresh: the old history drops with its
//! listener and the new controller tracks from its current value.
//! Restoration is a baseline, not an edit: binding rebaselines current and
//! clears both stacks instead of recording an undo step.
//! Reentrant edits made by listeners during notification commit normally —
//! newer wins and snapshots are never restored over them; edits made while
//! an undo/redo application is dispatching are absorbed into current without
//! creating steps, so current always tracks the editor.

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
    max_entries: usize,
}

const DEFAULT_MAX_ENTRIES: usize = 100;

fn trim_history(entries: &mut Vec<TextEditingValue>, max_entries: usize) {
    if entries.len() > max_entries {
        let remove = entries.len() - max_entries;
        entries.drain(..remove);
    }
}

struct Inner {
    editor: TextEditingController,
    history: RefCell<History>,
    applying: Cell<bool>,
    restoration_generation: Cell<u64>,
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
        // Restoration is a baseline, not an edit: rebaseline instead of
        // recording an undo step when the generation changed.
        let generation = self.editor.restoration_generation();
        if generation != self.restoration_generation.get() {
            self.restoration_generation.set(generation);
            let mut history = self.history.borrow_mut();
            history.current = value;
            history.undo.clear();
            history.redo.clear();
            drop(history);
            self.sync_state();
            return;
        }
        if self.applying.get() {
            self.history.borrow_mut().current = value;
            return;
        }

        let changed = {
            let mut history = self.history.borrow_mut();
            if history.current == value {
                false
            } else if history.current.text == value.text {
                // Selection/caret changes are part of the current editing
                // state, but should not create an undo step by themselves.
                history.current = value;
                false
            } else {
                let previous = std::mem::replace(&mut history.current, value);
                if history.max_entries > 0 {
                    history.undo.push(previous);
                    let max_entries = history.max_entries;
                    trim_history(&mut history.undo, max_entries);
                }
                history.redo.clear();
                true
            }
        };
        if changed {
            self.sync_state();
        }
    }

    fn apply_history_value(&self, value: TextEditingValue) {
        struct RestorePrevious<'a> {
            inner: &'a Inner,
            previous: bool,
        }

        impl Drop for RestorePrevious<'_> {
            fn drop(&mut self) {
                self.inner.history.borrow_mut().current = self.inner.editor.value();
                self.inner.applying.set(self.previous);
                if !std::thread::panicking() {
                    self.inner.sync_state();
                }
            }
        }

        let previous = self.applying.replace(true);
        let _guard = RestorePrevious {
            inner: self,
            previous,
        };
        self.editor.set_value(value);
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
                editor.add_listener(move |_| {
                    if let Some(inner) = weak.upgrade() {
                        inner.record(inner.editor.value());
                    }
                })
            };
            Inner {
                editor: editor.clone(),
                history: RefCell::new(History {
                    current: initial.clone(),
                    undo: Vec::new(),
                    redo: Vec::new(),
                    max_entries: DEFAULT_MAX_ENTRIES,
                }),
                applying: Cell::new(false),
                restoration_generation: Cell::new(editor.restoration_generation()),
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

    /// Limits the number of committed text states retained by this history.
    /// A value of zero disables recording while keeping the current editor
    /// value usable. Existing entries are trimmed immediately.
    pub fn set_max_entries(&self, max_entries: usize) {
        let mut history = self.inner.history.borrow_mut();
        history.max_entries = max_entries;
        trim_history(&mut history.undo, max_entries);
        trim_history(&mut history.redo, max_entries);
        drop(history);
        self.inner.sync_state();
    }

    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.inner.history.borrow().max_entries
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
            if history.max_entries > 0 {
                history.redo.push(current);
                let max_entries = history.max_entries;
                trim_history(&mut history.redo, max_entries);
            }
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
            if history.max_entries > 0 {
                history.undo.push(current);
                let max_entries = history.max_entries;
                trim_history(&mut history.undo, max_entries);
            }
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
