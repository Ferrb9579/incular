use incular_config::EdgeInsets;
use incular_controls::{
    ButtonStyle, ControlState, ControlTheme, ControlTypography, StateColor, StateTable, StateValue,
};
use incular_core::Color;

#[test]
fn explicit_literal_overrides_inherited_state_property() {
    let inherited = ButtonStyle::new().background_states(
        StateColor::new(Color::rgba(10, 20, 30, 255)).pressed(Color::rgba(30, 40, 50, 255)),
    );
    let explicit = ButtonStyle::new().background(Color::rgba(90, 80, 70, 255));
    let resolved = inherited.merge(&explicit);

    assert_eq!(
        resolved.resolve_background(ControlState::PRESSED, &ControlTheme::light()),
        Color::rgba(90, 80, 70, 255)
    );
    assert!(resolved.background_property.is_none());
    assert!(resolved.background_states.is_none());
}

#[test]
fn explicit_state_property_overrides_inherited_literal() {
    let inherited = ButtonStyle::new().background(Color::rgba(10, 20, 30, 255));
    let explicit = ButtonStyle::new().background_color(StateValue::states(
        StateTable::new(Color::rgba(40, 50, 60, 255)).invalid(Color::rgba(200, 20, 30, 255)),
    ));
    let resolved = inherited.merge(&explicit);

    assert_eq!(
        resolved.resolve_background(ControlState::INVALID, &ControlTheme::light()),
        Color::rgba(200, 20, 30, 255)
    );
    assert!(resolved.background.is_none());
    assert!(resolved.background_states.is_none());
}

#[test]
fn palette_change_preserves_explicit_typography() {
    let mut theme = ControlTheme::light();
    theme.typography = ControlTypography::new(Color::rgba(1, 2, 3, 255), Color::rgba(4, 5, 6, 255));
    let typography = theme.typography.clone();
    let palette = incular_controls::ControlColors::dark();

    let changed = theme.with_palette(palette.clone());
    assert_eq!(changed.typography, typography);
    assert_eq!(changed.colors, palette);
    assert_eq!(changed.palette(), &changed.colors);
}

#[test]
fn button_style_fluent_color_setters_leave_one_logical_representation() {
    let style = ButtonStyle::new()
        .background(Color::BLACK)
        .background_color(StateValue::states(StateTable::new(Color::WHITE)))
        .background_states(StateColor::new(Color::rgba(1, 2, 3, 255)))
        .foreground(Color::BLACK)
        .foreground_color(StateValue::states(StateTable::new(Color::WHITE)))
        .foreground_states(StateColor::new(Color::rgba(3, 2, 1, 255)))
        .padding(EdgeInsets::all(4.0));

    assert!(style.background.is_none());
    assert!(style.background_property.is_none());
    assert!(style.background_states.is_some());
    assert!(style.foreground.is_none());
    assert!(style.foreground_property.is_none());
    assert!(style.foreground_states.is_some());
}
