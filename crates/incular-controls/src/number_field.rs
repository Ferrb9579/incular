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
