use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use incular_core::{
    BuildContext, Color, HslColor, Key, KeyHandle, Offset, Rect, RestorationBackend,
    RestorationKey, RestorationKeyError, RestorationScope, Signal, Size, StringKey, Transform,
    UniqueKey, ValueKey, Widget,
};
use serde_json::{Value, json};

#[test]
fn signal_reads_register_and_writes_invalidate() {
    let context = BuildContext::new();
    let signal = Signal::new(1_u32);
    context.build(|_| assert_eq!(signal.get(), 1));
    assert_eq!(context.dependency_count(), 1);
    assert!(!context.is_dirty());
    assert!(signal.set(2));
    assert!(context.is_dirty());
    assert!(context.take_dirty());
    assert!(!context.is_dirty());
}

#[test]
fn inherited_values_track_the_environment_that_provided_them() {
    let root = BuildContext::new();
    let child = root.provide(7_u32);
    assert_eq!(child.build(|context| context.watch::<u32>()), Some(7));
    assert_eq!(child.dependency_count(), 1);
    assert!(!child.is_dirty());
    assert!(child.set(8_u32));
    assert!(child.is_dirty());
    assert_eq!(child.read::<u32>(), Some(8));
}

#[test]
fn nested_scopes_restore_the_outer_context() {
    let outer = BuildContext::new();
    let inner = BuildContext::new();
    let signal = Signal::new(3_u32);
    outer.run(|| {
        let _guard = inner.enter();
        assert_eq!(signal.get(), 3);
    });
    assert_eq!(outer.dependency_count(), 0);
    assert_eq!(inner.dependency_count(), 1);
}

#[test]
fn hsl_primary_colors_and_opacity_convert_to_srgb() {
    assert_eq!(
        HslColor::new(0.0, 1.0, 0.5, 1.0).to_color(),
        Color::rgba(255, 0, 0, 255)
    );
    assert_eq!(
        HslColor::new(120.0, 1.0, 0.5, 0.5).to_color(),
        Color::rgba(0, 255, 0, 128)
    );
    assert_eq!(
        HslColor::new(240.0, 1.0, 0.5, 1.0).to_color(),
        Color::rgba(0, 0, 255, 255)
    );
}

#[test]
fn affine_composition_inverse_and_bounds_use_kurbo() {
    let transform = Transform::translation(Offset::new(10., 20.))
        .then(Transform::rotation(std::f32::consts::FRAC_PI_2))
        .then(Transform::scale_non_uniform(2., 3.));
    let point = transform.transform_point(Offset::new(2., 0.));
    let restored = transform
        .inverse_transform_point(point)
        .expect("non-singular affine transform");
    assert!((restored.x - 2.).abs() < 0.0001);
    assert!(restored.y.abs() < 0.0001);

    let bounds = Transform::rotation(std::f32::consts::FRAC_PI_2)
        .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, Size::new(10., 20.)));
    assert!((bounds.size.width - 20.).abs() < 0.0001);
    assert!((bounds.size.height - 10.).abs() < 0.0001);
}

#[test]
fn hsv_round_trip_preserves_srgb_bytes() {
    for color in [
        Color::rgba(12, 190, 73, 64),
        Color::rgba(255, 128, 0, 255),
        Color::rgba(33, 33, 33, 0),
    ] {
        assert_eq!(color.to_hsv().to_color(), color);
    }
}

#[test]
fn hue_and_channels_are_sanitized() {
    let hsl = HslColor::new(-30.0, 4.0, -1.0, f32::NAN);
    assert_eq!(hsl.hue, 330.0);
    assert_eq!(hsl.saturation, 1.0);
    assert_eq!(hsl.lightness, 0.0);
    assert_eq!(hsl.alpha, 0.0);
}

#[test]
fn linear_rgba_uses_palette_srgb_transfer_function() {
    let [red, green, blue, alpha] = Color::rgba(128, 128, 128, 128).to_linear_rgba();
    assert!((red - 0.215_861).abs() < 0.000_01);
    assert_eq!(red, green);
    assert_eq!(green, blue);
    assert!((alpha - 128.0 / 255.0).abs() < f32::EPSILON);
}

#[test]
fn unique_keys_are_distinct_and_cloneable() {
    let first = UniqueKey::new();
    let second = UniqueKey::new();
    assert_ne!(first, second);
    assert_eq!(first, first);
    assert_eq!(KeyHandle::from(first), KeyHandle::from(first));
}

#[test]
fn value_keys_compare_by_value_after_erasure() {
    let first = KeyHandle::from(ValueKey::new(42_u32));
    let second = KeyHandle::from(ValueKey::new(42_u32));
    let other = KeyHandle::from(ValueKey::new(7_u32));
    assert_eq!(first, second);
    assert_ne!(first, other);
    assert_eq!(first.key_id(), second.key_id());
}

#[derive(Default)]
struct MemoryBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

impl RestorationBackend for MemoryBackend {
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

fn key(value: &str) -> RestorationKey {
    RestorationKey::new(value).unwrap()
}

#[test]
fn scopes_keep_path_segments_unambiguous() {
    let backend = Rc::new(MemoryBackend::default());
    let scope = RestorationScope::new(backend, [key("window/main"), key("editor")]);
    let value = key("document");

    scope.set_json(&value, json!({"body": "hello"}));

    assert_eq!(scope.path(), &[key("window/main"), key("editor")]);
    assert_eq!(scope.get_json(&value), Some(json!({"body": "hello"})));
}

#[test]
fn keys_validate_and_deserialize_through_the_same_rules() {
    assert_eq!(RestorationKey::new(""), Err(RestorationKeyError::Empty));
    assert_eq!(
        RestorationKey::new("contains\0nul"),
        Err(RestorationKeyError::ContainsNul)
    );
    assert!(serde_json::from_str::<RestorationKey>("\"\"").is_err());
    assert_eq!(
        serde_json::to_string(&key("main/sidebar")).unwrap(),
        "\"main/sidebar\""
    );
}

#[test]
fn scopes_can_remove_an_opt_in_value() {
    let backend = Rc::new(MemoryBackend::default());
    let scope = RestorationScope::new(backend, [key("application")]);
    let value = key("theme");
    scope.set_json(&value, json!("night"));
    scope.remove(&value);
    assert_eq!(scope.get_json(&value), None);
}

#[test]
fn restoration_scope_namespaces_children() {
    let backend = Rc::new(MemoryBackend::default());
    let root = RestorationScope::root(backend);
    assert!(root.is_root());
    let value = key("selection");
    let first = root.child(key("first"));
    let second = root.child(key("second"));
    first.set_json(&value, json!(1));
    second.set_json(&value, json!(2));
    assert_eq!(first.get_json(&value), Some(json!(1)));
    assert_eq!(second.get_json(&value), Some(json!(2)));
    assert!(first.path() != second.path());
}

#[derive(Debug)]
struct ExampleWidget {
    key: KeyHandle,
}

impl Widget for ExampleWidget {
    fn key(&self) -> Option<&dyn Key> {
        Some(self.key.as_key())
    }
}

#[test]
fn widget_contract_exposes_an_optional_key() {
    let widget = ExampleWidget {
        key: StringKey::new("home".to_owned()).into(),
    };
    assert_eq!(widget.key().expect("key").key_id(), widget.key.key_id());
}
