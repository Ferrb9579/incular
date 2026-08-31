use incular_core::Color;
use incular_material::{ColorScheme, MaterialTapTargetSize, ThemeData, ThemeDataPatch};

const THEME_CONSTRUCTION_STACK_BYTES: usize = 256 * 1024;

#[test]
fn theme_data_is_a_pointer_sized_shared_value() {
    assert_eq!(
        std::mem::size_of::<ThemeData>(),
        std::mem::size_of::<std::rc::Rc<()>>()
    );
}

#[test]
fn light_dark_and_seeded_themes_construct_on_small_native_stack() {
    // 256 KiB is Incular's regression budget for theme construction. It is
    // deliberately below typical native UI-thread stack reservations, so a
    // future field addition cannot silently recreate the former ~186 KiB
    // monolithic ThemeData stack pressure.
    std::thread::Builder::new()
        .stack_size(THEME_CONSTRUCTION_STACK_BYTES)
        .spawn(|| {
            let light = ThemeData::light();
            let dark = ThemeData::dark();
            let seeded = ThemeData::from_seed(Color::rgba(0x67, 0x50, 0xA4, 0xFF));
            let light_shared = ThemeData::light_shared();
            let dark_shared = ThemeData::dark_shared();
            let seeded_shared = ThemeData::from_seed_shared(Color::rgba(0x67, 0x50, 0xA4, 0xFF));

            assert_eq!(light.core().color_scheme, ColorScheme::light());
            assert_eq!(dark.core().color_scheme, ColorScheme::dark());
            assert_eq!(*light_shared, light);
            assert_eq!(*dark_shared, dark);
            assert_eq!(*seeded_shared, seeded);
        })
        .expect("spawn small-stack theme construction thread")
        .join()
        .expect("small-stack theme construction must not overflow or panic");
}

#[test]
fn copy_with_is_copy_on_write_and_preserves_unpatched_groups() {
    let original = ThemeData::light();
    let clone = original.clone();
    let patched = clone.copy_with(
        ThemeDataPatch::builder()
            .use_material3(false)
            .material_tap_target_size(MaterialTapTargetSize::ShrinkWrap)
            .build(),
    );

    assert!(original.core().use_material3);
    assert_eq!(
        original.core().material_tap_target_size,
        MaterialTapTargetSize::Padded
    );
    assert!(!patched.core().use_material3);
    assert_eq!(
        patched.core().material_tap_target_size,
        MaterialTapTargetSize::ShrinkWrap
    );
    assert_eq!(original.colors(), patched.colors());
    assert_eq!(original.buttons(), patched.buttons());
}

#[test]
fn shared_and_value_constructors_use_identical_defaults() {
    let schemes = [
        ColorScheme::light(),
        ColorScheme::dark(),
        ColorScheme::from_seed(Color::rgba(0x00, 0x78, 0xD4, 0xFF)),
    ];
    for scheme in schemes {
        assert_eq!(
            ThemeData::from_color_scheme_shared(scheme).as_ref(),
            &ThemeData::from_color_scheme(scheme)
        );
    }
}
