//! Number-field model and anatomy. The root owns one text editor/value model;
//! custom parts bind to that model through a typed retained scope.
use incular_widgets::internal::{ActionSurface, TextEditingController};
use incular_widgets::{EditableText, Row, Text, Widget};
use std::{cell::Cell, rc::Rc};
use typed_builder::TypedBuilder;

#[derive(Clone)]
struct NumberFieldScope {
    value: Rc<Cell<f64>>,
    min: Option<f64>,
    max: Option<f64>,
    step: f64,
    editor: TextEditingController,
    on_change: Option<Rc<dyn Fn(f64) + 'static>>,
    revision: Rc<Cell<u64>>,
}

impl NumberFieldScope {
    fn clamp(&self, value: f64) -> f64 {
        value
            .max(self.min.unwrap_or(f64::NEG_INFINITY))
            .min(self.max.unwrap_or(f64::INFINITY))
    }

    fn commit(&self, value: f64) {
        let next = self.clamp(value);
        if (self.value.get() - next).abs() <= f64::EPSILON {
            return;
        }
        self.value.set(next);
        self.revision.set(self.revision.get().wrapping_add(1));
        if let Some(callback) = self.on_change.as_ref() {
            callback(next);
        }
    }

    fn nudge(&self, direction: f64) {
        let next = self.clamp(self.value.get() + self.step * direction);
        if (self.value.get() - next).abs() <= f64::EPSILON {
            return;
        }
        self.editor.set_text(format_number(next));
        self.commit(next);
    }
}

fn format_number(value: f64) -> String {
    if value.fract().abs() <= f64::EPSILON {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn normalized_bounds(min: Option<f64>, max: Option<f64>) -> (Option<f64>, Option<f64>) {
    match (min, max) {
        (Some(a), Some(b)) if a > b => (Some(b), Some(a)),
        other => other,
    }
}

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
        self.normalize();
        self
    }
    #[must_use]
    pub fn max(mut self, value: f64) -> Self {
        self.max = Some(value);
        self.normalize();
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
    fn normalize(&mut self) {
        (self.min, self.max) = normalized_bounds(self.min, self.max);
        self.value = self.clamp(self.value);
    }
    fn clamp(&self, value: f64) -> f64 {
        value
            .max(self.min.unwrap_or(f64::NEG_INFINITY))
            .min(self.max.unwrap_or(f64::INFINITY))
    }
}

impl From<Root> for Widget {
    fn from(mut value: Root) -> Self {
        value.normalize();
        let editor = TextEditingController::with_text(format_number(value.value));
        let revision = Rc::new(Cell::new(0_u64));
        let scope = NumberFieldScope {
            value: Rc::new(Cell::new(value.value)),
            min: value.min,
            max: value.max,
            step: value.step,
            editor: editor.clone(),
            on_change: value.on_change,
            revision: revision.clone(),
        };
        let child = value.child.unwrap_or_else(|| {
            Row::new([
                Widget::from(Decrement::new(Text::new("−"))),
                Widget::from(Input::new(Text::new(format_number(value.value)))),
                Widget::from(Increment::new(Text::new("+"))),
            ])
            .into()
        });
        Widget::environment_scope(
            scope,
            Widget::stateful_layout_builder(revision, move |_, _| child.clone()),
        )
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
        let child = value.child;
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let Some(scope) = context.depend_on::<NumberFieldScope>() else {
                return child.clone();
            };
            let scope_for_edit = scope.clone();
            let editor: Widget = EditableText::new(scope.editor.clone()).into();
            let editor = editor.with_edit_callbacks(
                None,
                Some(Rc::new(move |text: &str| {
                    if let Ok(number) = text.trim().parse::<f64>() {
                        scope_for_edit.commit(number);
                    }
                })),
            );
            incular_widgets::Stack::new([
                child.clone(),
                incular_widgets::Positioned::fill(incular_widgets::Opacity::new(0.0, editor))
                    .into(),
            ])
            .into()
        }))
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
        action_part(value.child, 1.0)
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
        action_part(value.child, -1.0)
    }
}

fn action_part(child: Widget, direction: f64) -> Widget {
    Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
        let Some(scope) = context.depend_on::<NumberFieldScope>() else {
            return child.clone();
        };
        let callback_scope = scope.clone();
        ActionSurface::with_child(child.clone())
            .label(if direction > 0.0 {
                "Increment"
            } else {
                "Decrement"
            })
            .on_press(move || callback_scope.nudge(direction))
            .into()
    }))
}
