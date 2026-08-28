//! Number-field constraints and anatomy.
use incular_widgets::Widget;
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = 0.)]
    value: f64,
    #[builder(default, setter(strip_option))]
    min: Option<f64>,
    #[builder(default, setter(strip_option))]
    max: Option<f64>,
    #[builder(default = 1., setter(transform = |value: f64| value.abs().max(f64::EPSILON)))]
    step: f64,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(skip))]
    on_change: Option<Rc<dyn Fn(f64) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn value(mut self, value: f64) -> Self {
        self.value = self.clamp(value);
        self
    }
    #[must_use]
    pub fn min(mut self, value: f64) -> Self {
        self.min = Some(value);
        self.value = self.clamp(self.value);
        self
    }
    #[must_use]
    pub fn max(mut self, value: f64) -> Self {
        self.max = Some(value);
        self.value = self.clamp(self.value);
        self
    }
    #[must_use]
    pub fn step(mut self, value: f64) -> Self {
        self.step = value.abs().max(f64::EPSILON);
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, cb: impl Fn(f64) + 'static) -> Self {
        self.on_change = Some(Rc::new(cb));
        self
    }
    fn clamp(&self, value: f64) -> f64 {
        value
            .max(self.min.unwrap_or(f64::NEG_INFINITY))
            .min(self.max.unwrap_or(f64::INFINITY))
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Group {
    #[builder(setter(into))]
    child: Widget,
}
impl Group {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Input {
    #[builder(setter(into))]
    child: Widget,
}
impl Input {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Input> for Widget {
    fn from(value: Input) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Increment {
    #[builder(setter(into))]
    child: Widget,
}
impl Increment {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Increment> for Widget {
    fn from(value: Increment) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Decrement {
    #[builder(setter(into))]
    child: Widget,
}
impl Decrement {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}
impl From<Decrement> for Widget {
    fn from(value: Decrement) -> Self {
        value.child
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builders_use_explicit_defaults_and_widget_children() {
        let default = Root::builder().build();
        assert_eq!(default.value, 0.);
        assert!(default.min.is_none());
        assert!(default.max.is_none());
        assert_eq!(default.step, 1.);
        assert!(default.child.is_none());
        assert!(default.on_change.is_none());

        let root = Root::builder()
            .value(4.)
            .min(1.)
            .max(8.)
            .step(-2.)
            .child(Text::new("number"))
            .build();
        assert_eq!(root.value, 4.);
        assert_eq!(root.min, Some(1.));
        assert_eq!(root.max, Some(8.));
        assert_eq!(root.step, 2.);
        assert!(root.child.is_some());

        let _: Widget = Group::builder().child(Text::new("group")).build().into();
        let _: Widget = Input::builder().child(Text::new("input")).build().into();
        let _: Widget = Increment::builder()
            .child(Text::new("increment"))
            .build()
            .into();
        let _: Widget = Decrement::builder()
            .child(Text::new("decrement"))
            .build()
            .into();
    }
}
