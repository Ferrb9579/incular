//! Toolbar compound parts. Navigation is shared through CompositeController.
pub use crate::composite::CompositeController;
use incular_widgets::Widget;
#[derive(Clone)]
pub struct Root {
    child: Option<Widget>,
}
impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self { child: None }
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn navigation(&self) -> CompositeController {
        CompositeController::new()
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
macro_rules! toolbar_part {
    ($name:ident) => {
        #[derive(Clone)]
        pub struct $name {
            child: Widget,
        }
        impl $name {
            #[must_use]
            pub fn new(child: impl Into<Widget>) -> Self {
                Self {
                    child: child.into(),
                }
            }
        }
        impl From<$name> for Widget {
            fn from(value: $name) -> Self {
                value.child
            }
        }
    };
}
toolbar_part!(Button);
toolbar_part!(Input);
toolbar_part!(Link);
toolbar_part!(Group);
toolbar_part!(Separator);
