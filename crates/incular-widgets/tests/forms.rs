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

fn assert_complete_form_pass(save: bool) {
    for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
        let form = Form::new();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let saved = Rc::new(RefCell::new(Vec::new()));
        let controllers: Vec<_> = (0..3).map(|_| TextEditingController::new()).collect();
        let fields: Vec<_> = order
            .into_iter()
            .map(|index| {
                let calls = calls.clone();
                let saved = saved.clone();
                form.register(controllers[index].clone())
                    .validator(move |text| {
                        calls.borrow_mut().push(index);
                        text.is_empty().then(|| format!("required {index}"))
                    })
                    .on_saved(move |text| saved.borrow_mut().push((index, text)))
            })
            .collect();
        let notifications = Rc::new(Cell::new(0));
        let observer = form.observe({
            let notifications = notifications.clone();
            move || notifications.set(notifications.get() + 1)
        });
        for valid_count in 0..=3 {
            if valid_count > 0 {
                controllers[valid_count - 1].set_text("valid");
            }
            calls.borrow_mut().clear();
            let before = notifications.get();
            let valid = if save { form.save() } else { form.validate() };
            assert_eq!(valid, valid_count == 3);
            let mut actual_calls = calls.borrow().clone();
            actual_calls.sort_unstable();
            assert_eq!(
                actual_calls,
                vec![0, 1, 2],
                "every field validates exactly once"
            );
            assert_eq!(
                form.errors().len(),
                3 - valid_count,
                "clear stale errors as well as adding errors"
            );
            for (field, index) in fields.iter().zip(order) {
                assert_eq!(field.error().is_none(), index < valid_count);
            }
            assert_eq!(
                notifications.get(),
                before + 1,
                "one notification per form pass"
            );
            assert_eq!(saved.borrow().len(), if save && valid { 3 } else { 0 });
        }
        drop(observer);
    }
}

#[test]
fn form_pass_validate_visits_all_fields_and_clears_stale_errors() {
    assert_complete_form_pass(false);
}

#[test]
fn form_pass_save_validates_every_field_before_any_save_callback() {
    assert_complete_form_pass(true);
}

#[test]
fn form_validator_can_replace_itself_for_the_next_pass() {
    let form = Form::new();
    let registration = Rc::new(RefCell::new(None::<incular_widgets::FormField>));
    let weak = Rc::downgrade(&registration);
    let field = form
        .register(TextEditingController::new())
        .validator(move |_| {
            let slot = weak.upgrade().expect("registration is live");
            let field = slot.borrow_mut().take().expect("field installed");
            let field = field.validator(|_| None);
            slot.borrow_mut().replace(field);
            Some("original validator".into())
        });
    registration.borrow_mut().replace(field);
    assert!(!form.validate(), "current invocation keeps its result");
    assert!(
        form.validate(),
        "replacement is used on the next invocation"
    );
    assert!(form.errors().is_empty());
}

#[test]
fn form_save_callback_can_replace_itself_for_the_next_pass() {
    let form = Form::new();
    let registration = Rc::new(RefCell::new(None::<incular_widgets::FormField>));
    let weak = Rc::downgrade(&registration);
    let calls = Rc::new(Cell::new(0));
    let first_calls = calls.clone();
    let field = form
        .register(TextEditingController::new())
        .on_saved(move |_| {
            first_calls.set(first_calls.get() + 1);
            let slot = weak.upgrade().expect("registration is live");
            let field = slot.borrow_mut().take().expect("field installed");
            let next_calls = first_calls.clone();
            let field = field.on_saved(move |_| next_calls.set(next_calls.get() + 10));
            slot.borrow_mut().replace(field);
        });
    registration.borrow_mut().replace(field);
    assert!(form.save());
    assert_eq!(calls.get(), 1);
    assert!(form.save());
    assert_eq!(calls.get(), 11);
}
