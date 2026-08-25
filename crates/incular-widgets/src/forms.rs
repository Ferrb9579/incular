//! Form validation and autocomplete primitives.
//!
//! These types are deliberately controller-oriented.  A [`FormField`] owns
//! registration while it is mounted by application state, and can be paired
//! with any editor that uses a [`TextEditingController`].  This keeps form
//! state independent of a particular retained render node.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

use incular_core::{RestorationKey, RestorationScope};

use crate::{TextEditingController, Widget};

/// Stable identity for a field registered with a [`Form`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FormFieldId(u64);

/// When a field validates itself in response to user-originated editing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutovalidateMode {
    /// Validation only happens when [`Form::validate`] or
    /// [`FormField::validate`] is called.
    #[default]
    Disabled,
    /// Validate after [`FormField::set_text`] or
    /// [`FormField::notify_user_interaction`] is called.
    OnUserInteraction,
    /// Validate immediately when configured and after each user interaction.
    Always,
}

type Validator = Rc<dyn Fn(&str) -> Option<String>>;
type SubmitCallback = Rc<dyn Fn(String)>;

struct FieldState {
    controller: TextEditingController,
    initial_text: String,
    validator: RefCell<Option<Validator>>,
    on_submit: RefCell<Option<SubmitCallback>>,
    autovalidate: Cell<AutovalidateMode>,
    error: RefCell<Option<String>>,
}

#[derive(Default)]
struct FormState {
    next_id: u64,
    fields: HashMap<FormFieldId, Weak<FieldState>>,
}

/// Form-level validation and submit coordination.
#[derive(Clone, Default)]
pub struct Form {
    state: Rc<RefCell<FormState>>,
    child: Option<Widget>,
    autovalidate_mode: AutovalidateMode,
}

impl Form {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a declarative Form wrapping a child widget.
    #[must_use]
    pub fn of_child(child: impl Into<Widget>) -> Self {
        Self {
            state: Rc::new(RefCell::new(FormState::default())),
            child: Some(child.into()),
            autovalidate_mode: AutovalidateMode::Disabled,
        }
    }

    /// Sets the autovalidate mode for the form.
    #[must_use]
    pub fn autovalidate_mode(mut self, mode: AutovalidateMode) -> Self {
        self.autovalidate_mode = mode;
        self
    }

    /// Registers an editor with this form.  Keep the returned handle alive for
    /// as long as the corresponding UI field is mounted; dropping it removes
    /// the registration automatically.
    #[must_use]
    pub fn register(&self, controller: TextEditingController) -> FormField {
        let id = {
            let mut state = self.state.borrow_mut();
            state.next_id = state.next_id.saturating_add(1);
            FormFieldId(state.next_id)
        };
        let field = Rc::new(FieldState {
            initial_text: controller.text(),
            controller,
            validator: RefCell::new(None),
            on_submit: RefCell::new(None),
            autovalidate: Cell::new(AutovalidateMode::Disabled),
            error: RefCell::new(None),
        });
        self.state
            .borrow_mut()
            .fields
            .insert(id, Rc::downgrade(&field));
        FormField {
            id,
            field,
            form: Rc::downgrade(&self.state),
        }
    }

    /// Registers an editor and binds its committed value to a stable
    /// restoration key.
    ///
    /// Registration happens before binding so [`Self::reset`] still returns
    /// the caller's original default rather than the restored text.
    #[must_use]
    pub fn register_with_restoration(
        &self,
        controller: TextEditingController,
        scope: RestorationScope,
        key: RestorationKey,
    ) -> FormField {
        let field = self.register(controller);
        field.bind_restoration(scope, key);
        field
    }

    /// Registers an initially empty editor and restores it when a snapshot is
    /// available.
    #[must_use]
    pub fn register_restored(&self, scope: RestorationScope, key: RestorationKey) -> FormField {
        self.register_with_restoration(TextEditingController::new(), scope, key)
    }

    /// Validates every live field.  Fields that were unmounted are pruned as
    /// part of the operation, so forms cannot retain stale registrations.
    #[must_use]
    pub fn validate(&self) -> bool {
        self.live_fields()
            .into_iter()
            .all(|(_, field)| validate_field(&field))
    }

    /// Restores every live field to the value it had when registered.
    pub fn reset(&self) {
        for (_, field) in self.live_fields() {
            field.controller.set_text(field.initial_text.clone());
            field.error.replace(None);
        }
    }

    /// Validates the form then invokes each registered submit callback.  A
    /// callback is never invoked while any field is invalid.
    #[must_use]
    pub fn submit(&self) -> bool {
        let fields = self.live_fields();
        if !fields.iter().all(|(_, field)| validate_field(field)) {
            return false;
        }
        for (_, field) in fields {
            if let Some(callback) = field.on_submit.borrow().as_ref() {
                callback(field.controller.text());
            }
        }
        true
    }

    /// Returns the current errors keyed by stable field identity.
    #[must_use]
    pub fn errors(&self) -> Vec<(FormFieldId, String)> {
        self.live_fields()
            .into_iter()
            .filter_map(|(id, field)| field.error.borrow().clone().map(|error| (id, error)))
            .collect()
    }

    #[must_use]
    pub fn field_count(&self) -> usize {
        self.live_fields().len()
    }

    fn live_fields(&self) -> Vec<(FormFieldId, Rc<FieldState>)> {
        let mut state = self.state.borrow_mut();
        state.fields.retain(|_, field| field.strong_count() > 0);
        state
            .fields
            .iter()
            .filter_map(|(id, field)| field.upgrade().map(|field| (*id, field)))
            .collect()
    }
}

/// Registration and configuration handle for one text-backed form field.
pub struct FormField {
    id: FormFieldId,
    field: Rc<FieldState>,
    form: Weak<RefCell<FormState>>,
}

impl FormField {
    #[must_use]
    pub const fn id(&self) -> FormFieldId {
        self.id
    }

    /// Installs the field's validation function.  `Some(message)` represents
    /// a validation failure.
    #[must_use]
    pub fn validator(self, validator: impl Fn(&str) -> Option<String> + 'static) -> Self {
        self.field.validator.replace(Some(Rc::new(validator)));
        if self.field.autovalidate.get() == AutovalidateMode::Always {
            let _ = self.validate();
        }
        self
    }

    #[must_use]
    pub fn autovalidate(self, mode: AutovalidateMode) -> Self {
        self.field.autovalidate.set(mode);
        if mode == AutovalidateMode::Always {
            let _ = self.validate();
        }
        self
    }

    #[must_use]
    pub fn on_submit(self, callback: impl Fn(String) + 'static) -> Self {
        self.field.on_submit.replace(Some(Rc::new(callback)));
        self
    }

    /// Updates the backing editor and records user interaction for
    /// autovalidation.  Direct controller mutation remains supported for
    /// programmatic changes and does not imply user interaction.
    pub fn set_text(&self, text: impl Into<String>) {
        self.field.controller.set_text(text);
        self.notify_user_interaction();
    }

    /// Records an edit made through an externally-owned editor.
    pub fn notify_user_interaction(&self) {
        if self.field.autovalidate.get() != AutovalidateMode::Disabled {
            let _ = self.validate();
        }
    }

    #[must_use]
    pub fn validate(&self) -> bool {
        validate_field(&self.field)
    }

    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.field.error.borrow().clone()
    }

    /// Persists this field's committed editor value and selection under a
    /// stable key. Validation errors are derived from the current validator
    /// and are intentionally not persisted.
    pub fn bind_restoration(&self, scope: RestorationScope, key: RestorationKey) {
        self.field.controller.bind_restoration(scope, key);
    }

    /// Stops persisting subsequent editor changes without removing the saved
    /// value from the restoration manager.
    pub fn unbind_restoration(&self) {
        self.field.controller.unbind_restoration();
    }

    pub fn reset(&self) {
        self.field
            .controller
            .set_text(self.field.initial_text.clone());
        self.field.error.replace(None);
    }
}

impl Drop for FormField {
    fn drop(&mut self) {
        if let Some(form) = self.form.upgrade() {
            form.borrow_mut().fields.remove(&self.id);
        }
    }
}

fn validate_field(field: &FieldState) -> bool {
    let error = field
        .validator
        .borrow()
        .as_ref()
        .and_then(|validator| validator(&field.controller.text()));
    let valid = error.is_none();
    field.error.replace(error);
    valid
}

impl From<Form> for Widget {
    fn from(value: Form) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}

/// Generic FormField widget managing arbitrary typed form state and validation.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct GenericFormField<T: Clone + 'static> {
    initial_value: Option<T>,
    validator: Option<Rc<dyn Fn(&T) -> Option<String>>>,
    on_saved: Option<Rc<dyn Fn(T)>>,
    autovalidate_mode: AutovalidateMode,
    enabled: bool,
    restoration_id: Option<String>,
    builder: Rc<dyn Fn(Option<&T>) -> Widget>,
}

impl<T: Clone + 'static> GenericFormField<T> {
    #[must_use]
    pub fn new<W>(builder: impl Fn(Option<&T>) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            initial_value: None,
            validator: None,
            on_saved: None,
            autovalidate_mode: AutovalidateMode::Disabled,
            enabled: true,
            restoration_id: None,
            builder: Rc::new(move |val| builder(val).into()),
        }
    }

    #[must_use]
    pub fn initial_value(mut self, value: T) -> Self {
        self.initial_value = Some(value);
        self
    }

    #[must_use]
    pub fn validator(mut self, validator: impl Fn(&T) -> Option<String> + 'static) -> Self {
        self.validator = Some(Rc::new(validator));
        self
    }

    #[must_use]
    pub fn on_saved(mut self, on_saved: impl Fn(T) + 'static) -> Self {
        self.on_saved = Some(Rc::new(on_saved));
        self
    }

    #[must_use]
    pub fn autovalidate_mode(mut self, mode: AutovalidateMode) -> Self {
        self.autovalidate_mode = mode;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn restoration_id(mut self, id: impl Into<String>) -> Self {
        self.restoration_id = Some(id.into());
        self
    }
}

impl<T: Clone + 'static> From<GenericFormField<T>> for Widget {
    fn from(value: GenericFormField<T>) -> Self {
        (value.builder)(value.initial_value.as_ref())
    }
}

/// A core autocomplete widget coordinating text input with an options view.
#[derive(Clone)]
pub struct RawAutocomplete<T: Clone + 'static> {
    options: Vec<T>,
    display_string_for_option: Rc<dyn Fn(&T) -> String>,
    child: Option<Widget>,
}

impl<T: Clone + 'static> RawAutocomplete<T> {
    #[must_use]
    pub fn new(
        options: impl IntoIterator<Item = T>,
        display_string_for_option: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self {
            options: options.into_iter().collect(),
            display_string_for_option: Rc::new(display_string_for_option),
            child: None,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn options(&self) -> &[T] {
        &self.options
    }

    #[must_use]
    pub fn display_string(&self, option: &T) -> String {
        (self.display_string_for_option)(option)
    }
}

impl<T: Clone + 'static> From<RawAutocomplete<T>> for Widget {
    fn from(value: RawAutocomplete<T>) -> Self {
        value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into())
    }
}

/// Highlights the currently focused option in an autocomplete view.
#[derive(Clone, Debug, PartialEq)]
pub struct AutocompleteHighlightedOption {
    highlighted: bool,
    child: Widget,
}

impl AutocompleteHighlightedOption {
    #[must_use]
    pub fn new(highlighted: bool, child: impl Into<Widget>) -> Self {
        Self {
            highlighted,
            child: child.into(),
        }
    }
}

impl From<AutocompleteHighlightedOption> for Widget {
    fn from(value: AutocompleteHighlightedOption) -> Self {
        value.child
    }
}

/// Coordinates autofill context across text input descendants.
#[derive(Clone, Debug, PartialEq)]
pub struct AutofillGroup {
    child: Widget,
}

impl AutofillGroup {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

impl From<AutofillGroup> for Widget {
    fn from(value: AutofillGroup) -> Self {
        value.child
    }
}

/// Manages undo and redo history for editable text input.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoHistory {
    child: Widget,
}

impl UndoHistory {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}

/// State container passed to a [`GenericFormField`] builder function.
#[derive(Clone)]
pub struct FormFieldState<T> {
    value: Rc<RefCell<Option<T>>>,
    error_text: Rc<RefCell<Option<String>>>,
}

impl<T: Clone> FormFieldState<T> {
    #[must_use]
    pub fn value(&self) -> Option<T> {
        self.value.borrow().clone()
    }

    #[must_use]
    pub fn error_text(&self) -> Option<String> {
        self.error_text.borrow().clone()
    }

    #[must_use]
    pub fn has_error(&self) -> bool {
        self.error_text.borrow().is_some()
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.error_text.borrow().is_none()
    }

    pub fn did_change(&self, value: T) {
        *self.value.borrow_mut() = Some(value);
    }

    pub fn reset(&self, initial: Option<T>) {
        *self.value.borrow_mut() = initial;
        *self.error_text.borrow_mut() = None;
    }
}

/// A contract for formatting and validating live text edits.
pub trait TextInputFormatter {
    /// Formats an edit update from `old_value` to `new_value`.
    fn format_edit_update(
        &self,
        old_value: &crate::TextEditingValue,
        new_value: &crate::TextEditingValue,
    ) -> crate::TextEditingValue;
}

/// A text input formatter that filters characters using a predicate.
#[derive(Clone)]
pub struct FilteringTextInputFormatter {
    allow: bool,
    filter: Rc<dyn Fn(char) -> bool>,
}

impl FilteringTextInputFormatter {
    #[must_use]
    pub fn allow(filter: impl Fn(char) -> bool + 'static) -> Self {
        Self {
            allow: true,
            filter: Rc::new(filter),
        }
    }

    #[must_use]
    pub fn deny(filter: impl Fn(char) -> bool + 'static) -> Self {
        Self {
            allow: false,
            filter: Rc::new(filter),
        }
    }

    #[must_use]
    pub fn digits_only() -> Self {
        Self::allow(|c| c.is_ascii_digit())
    }

    #[must_use]
    pub fn single_line_formatter() -> Self {
        Self::deny(|c| c == '\n' || c == '\r')
    }
}

impl TextInputFormatter for FilteringTextInputFormatter {
    fn format_edit_update(
        &self,
        _old_value: &crate::TextEditingValue,
        new_value: &crate::TextEditingValue,
    ) -> crate::TextEditingValue {
        let filtered: String = new_value
            .text
            .chars()
            .filter(|&c| {
                if self.allow {
                    (self.filter)(c)
                } else {
                    !(self.filter)(c)
                }
            })
            .collect();
        crate::TextEditingValue {
            text: filtered,
            selection: new_value.selection,
            preedit: new_value.preedit.clone(),
            preedit_selection: new_value.preedit_selection,
        }
    }
}

/// Enforcement policy for maximum text length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MaxLengthEnforcement {
    None,
    #[default]
    Enforced,
    TruncateAfterCompositionEnds,
}

/// A text input formatter that limits input length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LengthLimitingTextInputFormatter {
    pub max_length: usize,
    pub max_length_enforcement: MaxLengthEnforcement,
}

impl LengthLimitingTextInputFormatter {
    #[must_use]
    pub const fn new(max_length: usize) -> Self {
        Self {
            max_length,
            max_length_enforcement: MaxLengthEnforcement::Enforced,
        }
    }

    #[must_use]
    pub const fn with_enforcement(max_length: usize, enforcement: MaxLengthEnforcement) -> Self {
        Self {
            max_length,
            max_length_enforcement: enforcement,
        }
    }
}

impl TextInputFormatter for LengthLimitingTextInputFormatter {
    fn format_edit_update(
        &self,
        old_value: &crate::TextEditingValue,
        new_value: &crate::TextEditingValue,
    ) -> crate::TextEditingValue {
        if new_value.text.chars().count() <= self.max_length {
            new_value.clone()
        } else {
            let truncated: String = new_value.text.chars().take(self.max_length).collect();
            crate::TextEditingValue {
                text: truncated,
                selection: old_value.selection,
                preedit: None,
                preedit_selection: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        rc::Rc,
    };

    use serde_json::{Value, json};

    #[derive(Default)]
    struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

    impl incular_core::RestorationBackend for MemoryRestorationBackend {
        fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
            self.0.borrow().get(path).cloned()
        }

        fn write_value(&self, path: &[RestorationKey], value: Value) {
            self.0.borrow_mut().insert(path.to_vec(), value);
        }

        fn remove_value(&self, path: &[RestorationKey]) {
            self.0.borrow_mut().remove(path);
        }
    }

    fn restoration_key(value: &str) -> RestorationKey {
        RestorationKey::new(value).unwrap()
    }

    fn restoration_scope() -> RestorationScope {
        RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
            .child_unchecked(restoration_key("window"))
            .child_unchecked(restoration_key("main"))
    }

    #[test]
    fn form_validates_autovalidates_submits_and_resets() {
        let form = Form::new();
        let controller = TextEditingController::with_text("Ada");
        let submitted = Rc::new(Cell::new(0));
        let field = form
            .register(controller.clone())
            .validator(|value| (!value.contains('@')).then_some("email required".into()))
            .autovalidate(AutovalidateMode::OnUserInteraction)
            .on_submit({
                let submitted = submitted.clone();
                move |_| submitted.set(submitted.get() + 1)
            });
        assert!(!form.validate());
        assert_eq!(field.error().as_deref(), Some("email required"));
        assert!(!form.submit());
        field.set_text("ada@example.test");
        assert!(field.error().is_none());
        assert!(form.submit());
        assert_eq!(submitted.get(), 1);
        form.reset();
        assert_eq!(controller.text(), "Ada");
    }

    #[test]
    fn form_unregisters_dropped_fields() {
        let form = Form::new();
        let field = form.register(TextEditingController::new());
        assert_eq!(form.field_count(), 1);
        drop(field);
        assert_eq!(form.field_count(), 0);
    }

    #[test]
    fn form_field_restoration_keeps_the_declared_reset_default() {
        let scope = restoration_scope();
        let key = restoration_key("email");
        scope.set_json(
            &key,
            json!({
                "text": "restored@example.test",
                "selection": { "base": 8, "extent": 8 },
            }),
        );

        let form = Form::new();
        let controller = TextEditingController::with_text("default@example.test");
        let field = form.register_with_restoration(controller.clone(), scope.clone(), key.clone());

        assert_eq!(controller.text(), "restored@example.test");
        field.set_text("updated@example.test");
        assert_eq!(
            scope.get_json(&key),
            Some(json!({
                "text": "updated@example.test",
                "selection": { "base": 20, "extent": 20 },
            }))
        );
        field.reset();
        assert_eq!(controller.text(), "default@example.test");
    }

    #[test]
    fn restored_form_recomputes_validation_instead_of_reusing_stale_presentation() {
        let scope = restoration_scope();
        let key = restoration_key("email");
        scope.set_json(
            &key,
            json!({
                "text": "invalid",
                "selection": { "base": 7, "extent": 7 },
                "error": "obsolete validator message",
            }),
        );

        let field = Form::new()
            .register_restored(scope, key)
            .validator(|value| (!value.contains('@')).then_some("email is required".into()))
            .autovalidate(AutovalidateMode::Always);

        assert_eq!(field.error().as_deref(), Some("email is required"));
    }
}
