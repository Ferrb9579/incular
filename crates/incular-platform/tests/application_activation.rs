use incular_core::{Code, Modifiers};
use incular_platform::{
    ApplicationActivation, GlobalShortcutChord, GlobalShortcutChordError, SingleInstancePolicy,
};
use std::path::PathBuf;
use url::Url;

#[test]
fn open_file_activation_preserves_exact_paths_and_order() {
    let first = PathBuf::from(r"C:\Users\winter\notes one.txt");
    let second = PathBuf::from(r"D:\数据\draft.ram");
    let activation = ApplicationActivation::open_files([first.clone(), second.clone()]);
    let paths = activation
        .documents()
        .iter()
        .map(|document| document.path().to_path_buf())
        .collect::<Vec<_>>();
    assert_eq!(paths, vec![first, second]);
}

#[test]
fn url_activation_preserves_order_without_route_interpretation() {
    let first = Url::parse("rambler://capture/new?mode=voice").expect("first URL");
    let second = Url::parse("rambler://settings/audio#input").expect("second URL");
    let activation = ApplicationActivation::open_urls([first.clone(), second.clone()]);
    assert_eq!(activation.urls(), &[first, second]);
}

#[test]
fn global_shortcut_chord_rejects_modifier_keys_and_non_chord_modifier_state() {
    assert_eq!(
        GlobalShortcutChord::new(Code::ControlLeft, Modifiers::CONTROL),
        Err(GlobalShortcutChordError::InvalidKey(Code::ControlLeft))
    );
    assert_eq!(
        GlobalShortcutChord::new(Code::KeyR, Modifiers::CONTROL | Modifiers::CAPS_LOCK),
        Err(GlobalShortcutChordError::UnsupportedModifiers(
            Modifiers::CONTROL | Modifiers::CAPS_LOCK
        ))
    );
    assert!(GlobalShortcutChord::new(Code::KeyR, Modifiers::CONTROL | Modifiers::SHIFT).is_ok());
}

#[test]
fn single_instance_identity_is_stable_and_rejects_path_like_values() {
    let policy = SingleInstancePolicy::new("dev.winterhoax.rambler").expect("valid app id");
    assert_eq!(policy.application_id(), "dev.winterhoax.rambler");
    assert!(SingleInstancePolicy::new("../rambler").is_err());
    assert!(SingleInstancePolicy::new("").is_err());
}
