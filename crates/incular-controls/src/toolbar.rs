//! Toolbar compound parts. Navigation is shared through CompositeController.
pub use crate::composite::CompositeController;
use incular_widgets::Widget;
use typed_builder::TypedBuilder;

#[derive(Clone, Default, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
        #[derive(Clone, TypedBuilder)]
        pub struct $name {
            #[builder(setter(into))]
            child: Widget,
        }
        impl $name {
            #[must_use]
            pub fn new(child: impl Into<Widget>) -> Self {
                Self::builder().child(child).build()
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn root_builder_accepts_a_generic_child() {
        let root = Root::builder().child(Text::new("toolbar")).build();
        let _: Widget = root.into();
        let _: Widget = Root::default().into();
    }

    #[test]
    fn toolbar_parts_expose_typed_builders_and_preserve_constructors() {
        let _: Widget = Button::builder().child(Text::new("button")).build().into();
        let _: Widget = Input::builder().child(Text::new("input")).build().into();
        let _: Widget = Link::builder().child(Text::new("link")).build().into();
        let _: Widget = Group::builder().child(Text::new("group")).build().into();
        let _: Widget = Separator::builder()
            .child(Text::new("separator"))
            .build()
            .into();

        let _: Widget = Button::new(Text::new("new button")).into();
    }
}
