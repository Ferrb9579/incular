use incular_config::{Brightness, EdgeInsets, RuntimeEnvironment};
use incular_platform::{
    MemorySystemEnvironmentProvider, SystemEnvironmentPreferences, SystemEnvironmentProvider,
    canonicalize_system_locales,
};

#[test]
fn memory_provider_replaces_complete_snapshots_deterministically() {
    let provider = MemorySystemEnvironmentProvider::new(SystemEnvironmentPreferences {
        reduced_motion: Some(true),
        high_contrast: Some(true),
        ..SystemEnvironmentPreferences::default()
    });

    assert!(
        provider
            .snapshot()
            .reduced_motion
            .is_some_and(|value| value)
    );
    assert!(!provider.replace(provider.snapshot()));
    assert!(provider.replace(SystemEnvironmentPreferences {
        reduced_motion: Some(false),
        ..SystemEnvironmentPreferences::default()
    }));
    assert_eq!(provider.snapshot().reduced_motion, Some(false));
    assert_eq!(provider.snapshot().high_contrast, None);
}

#[test]
fn unsupported_values_reset_to_documented_runtime_defaults() {
    let mut environment = RuntimeEnvironment {
        brightness: Brightness::Dark,
        reduced_motion: true,
        high_contrast: true,
        text_scale: 1.75,
        safe_insets: EdgeInsets::all(9.0),
        ..RuntimeEnvironment::default()
    };

    SystemEnvironmentPreferences::default().apply_to(&mut environment);
    let defaults = RuntimeEnvironment::default();
    assert_eq!(environment.brightness, Brightness::Dark);
    assert_eq!(environment.reduced_motion, defaults.reduced_motion);
    assert_eq!(environment.high_contrast, defaults.high_contrast);
    assert_eq!(environment.text_scale, defaults.text_scale);
    assert_eq!(environment.safe_insets, defaults.safe_insets);
    assert_eq!(environment.locales, defaults.locales);
}

#[test]
fn system_locales_preserve_preference_order_and_drop_invalid_duplicates() {
    let locales = canonicalize_system_locales(["ar-EG", "en-US", "ar-EG", "@@@"]);
    let values = locales
        .into_iter()
        .map(|locale| locale.to_string())
        .collect::<Vec<_>>();
    assert_eq!(values, vec!["ar-EG", "en-US"]);
}
