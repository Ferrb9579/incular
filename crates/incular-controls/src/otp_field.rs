//! One retained editor with fixed visual slots, rather than one editor per
//! digit. This keeps paste, selection, and accessibility coherent.
use incular_widgets::Widget;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = 6, setter(transform = |length: usize| length.max(1)))]
    length: usize,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default)]
    masked: bool,
}
impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Root {
    #[must_use]
    pub fn new(length: usize) -> Self {
        Self::builder().length(length).build()
    }
    #[must_use]
    pub fn length(&self) -> usize {
        self.length
    }
    #[must_use]
    pub fn is_masked(&self) -> bool {
        self.masked
    }
    #[must_use]
    pub fn masked(mut self, value: bool) -> Self {
        self.masked = value;
        self
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
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
