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

use crate::TextEditingController;

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
}

impl Form {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

/// A synchronous autocomplete model that is independent of a particular text
/// field presentation.  Applications can render [`Self::suggestions`] with a
/// list, popup, or inline controls and feed the selected value into any
/// editor.
pub struct Autocomplete<T> {
    options: Vec<T>,
    label: Rc<dyn Fn(&T) -> String>,
    query: String,
    selected: Option<usize>,
}

impl<T> Autocomplete<T> {
    #[must_use]
    pub fn new(
        options: impl IntoIterator<Item = T>,
        label: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self {
            options: options.into_iter().collect(),
            label: Rc::new(label),
            query: String::new(),
            selected: None,
        }
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = None;
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns matching option indices in source order.  Matching is
    /// case-insensitive and intentionally allocation-free for callers that
    /// keep option values outside the model.
    #[must_use]
    pub fn matching_indices(&self) -> Vec<usize> {
        let needle = self.query.to_lowercase();
        self.options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                ((self.label)(option).to_lowercase().contains(&needle)).then_some(index)
            })
            .collect()
    }

    #[must_use]
    pub fn option(&self, index: usize) -> Option<&T> {
        self.options.get(index)
    }

    #[must_use]
    pub fn select(&mut self, index: usize) -> Option<&T> {
        self.options.get(index)?;
        self.selected = Some(index);
        self.options.get(index)
    }

    #[must_use]
    pub fn selected(&self) -> Option<&T> {
        self.selected.and_then(|index| self.options.get(index))
    }
}

impl Autocomplete<String> {
    /// Convenience constructor for the common string option case.
    #[must_use]
    pub fn strings(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::new(options.into_iter().map(Into::into), Clone::clone)
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

    #[test]
    fn autocomplete_filters_in_source_order_and_tracks_selection() {
        let mut autocomplete = Autocomplete::strings(["Ada", "Grace", "Alan"]);
        autocomplete.set_query("a");
        assert_eq!(autocomplete.matching_indices(), vec![0, 1, 2]);
        assert_eq!(autocomplete.select(1), Some(&"Grace".to_owned()));
        assert_eq!(autocomplete.selected(), Some(&"Grace".to_owned()));
        autocomplete.set_query("al");
        assert_eq!(autocomplete.matching_indices(), vec![2]);
        assert!(autocomplete.selected().is_none());
    }
}
