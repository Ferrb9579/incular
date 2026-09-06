//! Form validation, persistence, and restoration behavior tests.

use incular_core::{RestorationKey, RestorationScope};
use incular_text::TextEditingController;
use incular_widgets::{AutovalidateMode, Form, FormState};

#[test]
fn form_errors_track_and_field_drop_removes_its_controller_observer() {
    let context = incular_core::BuildContext::new();
    let form = Form::new();
    let controller = TextEditingController::with_text("Ada");
    let field = form
        .register(controller.clone())
        .validator(|text| text.is_empty().then(|| "required".to_owned()))
        .autovalidate(AutovalidateMode::Always);
    context.build(|_| {
        assert!(form.errors().is_empty());
    });
    controller.set_text("");
    assert!(context.take_dirty());
    assert_eq!(form.errors().len(), 1);
    drop(field);
    assert!(context.take_dirty());
    assert_eq!(form.field_count(), 0);
    controller.set_text("Grace");
    assert!(!context.is_dirty());
}
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

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
fn form_validate_save_reset() {
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
fn form_controller_survives_rebuild() {
    let form: FormState = Form::new();
    let controller = TextEditingController::with_text("before");
    let saved = Rc::new(Cell::new(0));
    let field = form.register(controller.clone()).on_saved({
        let saved = saved.clone();
        move |_| saved.set(saved.get() + 1)
    });

    let rebuilt = form.clone();
    field.set_text("after");
    assert!(rebuilt.save());
    assert_eq!(saved.get(), 1);
    rebuilt.reset();
    assert_eq!(controller.text(), "before");
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
