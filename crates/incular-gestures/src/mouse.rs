use std::cell::Cell;
use std::rc::Rc;

use incular_core::Offset;

/// Pointer hover callbacks for desktop-capable input sources.
#[derive(Clone, Default)]
pub struct MouseRegion {
    pub on_enter: Option<Rc<dyn Fn(Offset)>>,
    pub on_exit: Option<Rc<dyn Fn(Offset)>>,
    pub on_hover: Option<Rc<dyn Fn(Offset)>>,
    inside: Rc<Cell<bool>>,
}
impl MouseRegion {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn enter(&self, position: Offset) {
        if !self.inside.replace(true)
            && let Some(callback) = &self.on_enter
        {
            callback(position);
        }
    }
    pub fn hover(&self, position: Offset) {
        if self.inside.get()
            && let Some(callback) = &self.on_hover
        {
            callback(position);
        }
    }
    pub fn exit(&self, position: Offset) {
        if self.inside.replace(false)
            && let Some(callback) = &self.on_exit
        {
            callback(position);
        }
    }
    #[must_use]
    pub fn is_inside(&self) -> bool {
        self.inside.get()
    }
}
