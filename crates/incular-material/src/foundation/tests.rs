use super::*;
use incular_config::Constraints;
use incular_core::{Color, Size};

#[test]
fn state_property_prefers_more_specific_state() {
    let property =
        StateProperty::from_map([(WidgetState::Hovered, 1_u32), (WidgetState::Pressed, 2_u32)]);
    let states = WidgetStates::new()
        .with(WidgetState::Hovered)
        .with(WidgetState::Pressed);
    assert_eq!(property.resolve(states), 2);
}

#[test]
fn visual_density_adjusts_both_axes() {
    let constraints = Constraints::tight(Size::new(48.0, 48.0));
    let compact = VisualDensity::COMPACT.effective_constraints(constraints);
    assert_eq!(compact.min_width, 40.0);
    assert_eq!(compact.min_height, 40.0);
}

#[test]
fn seeded_scheme_has_complete_surface_roles() {
    let scheme = ColorScheme::from_seed(Color::rgba(0x67, 0x50, 0xa4, 255));
    assert_eq!(scheme.background, scheme.surface);
    assert_eq!(scheme.on_background, scheme.on_surface);
    assert_ne!(scheme.primary, Color::TRANSPARENT);
    assert_ne!(scheme.surface_container_highest, Color::TRANSPARENT);
}

#[test]
fn theme_precedence_is_sparse() {
    let theme = ThemeData::light().copy_with(ThemeDataPatch {
        use_material3: Some(false),
        ..ThemeDataPatch::default()
    });
    assert!(!theme.use_material3);
    assert_eq!(theme.color_scheme, ColorScheme::light());
}
