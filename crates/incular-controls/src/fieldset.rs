//! Fieldset/legend semantic grouping.
use incular_widgets::Widget;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default = false)]
    disabled: bool,
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
#[derive(Clone, TypedBuilder)]
pub struct Legend {
    #[builder(setter(into))]
    child: Widget,
}
impl Legend {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self::builder()
            .child(incular_widgets::Text::new(value))
            .build()
    }
}
impl From<Legend> for Widget {
    fn from(value: Legend) -> Self {
        value.child
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builders_preserve_fieldset_defaults_and_accept_generic_children() {
        let default = Root::default();
        let built = Root::builder().child(Text::new("Fields")).build();

        assert!(default.child.is_none());
        assert!(!default.disabled);
        assert!(built.child.is_some());
        assert!(!built.disabled);

        let legend = Legend::builder().child(Text::new("Name")).build();
        assert_eq!(legend.child.text_if_any().as_deref(), Some("Name"));
        let _: Widget = Root::new().disabled(true).into();
        let _: Widget = Legend::new("Name").into();
    }
}
