//! One retained editor with fixed visual slots, rather than one editor per
//! digit. This keeps paste, selection, and accessibility coherent.
use incular_widgets::Widget;
#[derive(Clone)]
pub struct Root {
    length: usize,
    child: Option<Widget>,
    masked: bool,
}
impl Default for Root {
    fn default() -> Self {
        Self::new(6)
    }
}
impl Root {
    #[must_use]
    pub fn new(length: usize) -> Self {
        Self {
            length: length.max(1),
            child: None,
            masked: false,
        }
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
