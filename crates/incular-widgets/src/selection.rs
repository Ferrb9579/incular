//! Retained coordination state for read-only text selection.
//!
//! The widget tree owns hit testing and shaped-layout geometry. This small
//! controller deliberately owns only observable selection output, so copying
//! text never turns a static label into an editable buffer.

use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
pub struct SelectionAreaController {
    state: Rc<RefCell<SelectionAreaState>>,
}

#[derive(Default)]
struct SelectionAreaState {
    selected_text: String,
    revision: u64,
}

impl std::fmt::Debug for SelectionAreaController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectionAreaController")
            .field("selected_text", &self.selected_text())
            .field("revision", &self.revision())
            .finish()
    }
}

impl PartialEq for SelectionAreaController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl SelectionAreaController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Text currently selected in this area, suitable for a platform clipboard.
    #[must_use]
    pub fn selected_text(&self) -> String {
        self.state.borrow().selected_text.clone()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state.borrow().selected_text.is_empty()
    }

    /// Monotonically changes whenever pointer or keyboard selection changes.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    pub(crate) fn set_selected_text(&self, text: String) {
        let mut state = self.state.borrow_mut();
        if state.selected_text != text {
            state.selected_text = text;
            state.revision = state.revision.wrapping_add(1);
        }
    }
}
