//! Fieldset/legend semantic grouping.
use incular_widgets::Widget;
#[derive(Clone)]
pub struct Root {
    child: Option<Widget>,
    disabled: bool,
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
            child: None,
            disabled: false,
        }
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
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
pub struct Legend {
    child: Widget,
}
impl Legend {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            child: incular_widgets::Text::new(value).into(),
        }
    }
}
impl From<Legend> for Widget {
    fn from(value: Legend) -> Self {
        value.child
    }
}
