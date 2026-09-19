//! One retained editor with fixed visual slots, rather than one editor per
//! digit. This keeps paste, selection, and accessibility coherent.
use incular_widgets::internal::TextEditingController;
use incular_widgets::{
    EditableText, LengthLimitingTextInputFormatter, Opacity, Positioned, Stack, TextInputFormatter,
    Widget,
};
use std::rc::Rc;
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
        let controller = TextEditingController::new();
        let limiter = LengthLimitingTextInputFormatter::new(value.length);
        let mut editor: Widget = EditableText::new(controller)
            .obscure_text(value.masked)
            .into();
        editor = editor.with_edit_callbacks(
            Some(Rc::new(move |old, next| {
                limiter.format_edit_update(old, next)
            })),
            None,
        );
        match value.child {
            None => editor,
            Some(visual) => {
                Stack::new([visual, Positioned::fill(Opacity::new(0.0, editor)).into()]).into()
            }
        }
    }
}
