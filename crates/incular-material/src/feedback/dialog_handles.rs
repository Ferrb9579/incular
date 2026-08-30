use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::Dialog;
use incular_controls::current_control_theme;
use incular_core::Color;
use incular_widgets::internal::ActionSurface;
use incular_widgets::{Container, OverlayPortal, Positioned, SizedBox, Stack, Widget};

/// A retained, local dialog presentation handle.
///
/// The handle is intentionally independent of a navigator.  It can be placed
/// over an arbitrary child with [`DialogHandle::present`], while a navigation
/// implementation may use the same `Dialog` descriptor in its own route.
#[derive(Clone)]
pub struct DialogHandle {
    dialog: Dialog,
    open: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    barrier_dismissible: bool,
    barrier_color: Color,
}

/// Rust-native spelling of Flutter's `DialogRoute`.
///
/// The retained handle is the route/presentation state in Incular: it owns
/// barrier policy and lifecycle while the runtime/navigation layer remains in
/// charge of scheduling and route stacks.  Keeping this alias means code that
/// names a dialog route can migrate without introducing a second route type.
pub type DialogRoute = DialogHandle;

impl DialogHandle {
    #[must_use]
    pub fn new(dialog: Dialog) -> Self {
        Self {
            dialog,
            open: Rc::new(Cell::new(false)),
            revision: Rc::new(Cell::new(0)),
            barrier_dismissible: true,
            barrier_color: Color::rgba(0, 0, 0, 96),
        }
    }

    #[must_use]
    pub fn dialog(&self) -> Dialog {
        self.dialog.clone()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn open(&self) {
        self.set_open(true);
    }

    pub fn close(&self) {
        self.set_open(false);
    }

    /// Alias for [`DialogHandle::close`].
    pub fn dismiss(&self) {
        self.close();
    }

    #[must_use]
    pub fn barrier_dismissible(mut self, dismissible: bool) -> Self {
        self.barrier_dismissible = dismissible;
        self
    }

    #[must_use]
    pub fn barrier_color(mut self, color: Color) -> Self {
        self.barrier_color = color;
        self
    }

    fn set_open(&self, open: bool) {
        if self.open.replace(open) != open {
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }

    fn build_overlay(&self) -> Widget {
        let dismiss = self.clone();
        let barrier = ActionSurface::with_child(Container::new().color(self.barrier_color))
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .focused_color(Color::TRANSPARENT)
            .enabled(self.barrier_dismissible)
            .focusable_when_disabled(false);
        let barrier = if self.barrier_dismissible {
            barrier.on_click(move || dismiss.dismiss())
        } else {
            barrier
        };
        let barrier: Widget = Positioned::fill(barrier).into();
        Stack::new([barrier, self.dialog.build(&current_control_theme())]).into()
    }

    /// Presents the dialog over `child` when this handle is open.
    #[must_use]
    pub fn present(&self, child: impl Into<Widget>) -> Widget {
        let child = child.into();
        let handle = self.clone();
        Widget::stateful_layout_builder(self.revision.clone(), move |_| {
            let base = OverlayPortal::new(child.clone())
                .overlay_child(handle.build_overlay())
                .show(handle.is_open());
            let presented: Widget = base.into();
            if handle.is_open() {
                presented.block_semantics()
            } else {
                presented
            }
        })
    }

    /// Returns a presentation rooted at an empty child.
    #[must_use]
    pub fn widget(&self) -> Widget {
        self.present(SizedBox::shrink())
    }
}

/// Creates a local retained dialog presentation descriptor.
#[must_use]
pub fn show_dialog(dialog: Dialog) -> DialogHandle {
    DialogHandle::new(dialog)
}

/// A typed result handle for callers that want a dialog result without tying
/// the feedback layer to a particular navigation/future implementation.
#[derive(Clone)]
pub struct DialogResultHandle<T> {
    handle: DialogHandle,
    result: Rc<RefCell<Option<T>>>,
}

impl<T> DialogResultHandle<T> {
    #[must_use]
    pub fn handle(&self) -> DialogHandle {
        self.handle.clone()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    pub fn open(&self) {
        self.handle.open();
    }

    pub fn close(&self) {
        self.handle.close();
    }

    pub fn complete(&self, value: T) {
        *self.result.borrow_mut() = Some(value);
        self.handle.close();
    }

    #[must_use]
    pub fn take_result(&self) -> Option<T> {
        self.result.borrow_mut().take()
    }

    #[must_use]
    pub fn present(&self, child: impl Into<Widget>) -> Widget {
        self.handle.present(child)
    }
}

/// Creates a typed local dialog result descriptor.
#[must_use]
pub fn show_dialog_result<T>(dialog: Dialog) -> DialogResultHandle<T> {
    DialogResultHandle {
        handle: DialogHandle::new(dialog),
        result: Rc::new(RefCell::new(None)),
    }
}
