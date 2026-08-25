//! Number-field constraints and anatomy.
use incular_widgets::Widget;
use std::rc::Rc;
#[derive(Clone)]
pub struct Root {
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
    step: f64,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(f64) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: 0.,
            min: None,
            max: None,
            step: 1.,
            child: None,
            on_change: None,
        }
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
#[derive(Clone)]
pub struct Group {
    child: Widget,
}
impl Group {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Input {
    child: Widget,
}
impl Input {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Input> for Widget {
    fn from(value: Input) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Increment {
    child: Widget,
}
impl Increment {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Increment> for Widget {
    fn from(value: Increment) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Decrement {
    child: Widget,
}
impl Decrement {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Decrement> for Widget {
    fn from(value: Decrement) -> Self {
        value.child
    }
}
