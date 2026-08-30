use super::style::ButtonKind;
use super::{
    ButtonStyleConfig, ElevatedButton, FilledButton, FloatingActionButton, IconButton,
    OutlinedButton, TextButton,
};
use crate::foundation::ThemeData;
use incular_controls::{ButtonVariant, ControlState, StateTable, StateValue, TapTargetSize};
use incular_core::{Color, Size};
use incular_widgets::{SizedBox, Widget};

#[test]
fn style_factory_preserves_sparse_state_properties() {
    let style = ElevatedButton::style_from(
        ButtonStyleConfig::new()
            .background_color(Color::rgba(1, 2, 3, 255))
            .elevation(StateValue::states(StateTable::new(1.0).pressed(4.0)))
            .minimum_size(Size::new(80.0, 40.0)),
    );
    assert_eq!(style.background, Some(Color::rgba(1, 2, 3, 255)));
    assert_eq!(style.minimum_size, Some(Size::new(80.0, 40.0)));
    assert_eq!(
        style
            .elevation
            .as_ref()
            .expect("elevation")
            .resolve(ControlState::PRESSED),
        4.0
    );
    assert_eq!(style.variant, ButtonVariant::Primary);
}

#[test]
fn button_families_have_widget_conversions() {
    let _: Widget = ElevatedButton::new("Elevated").into();
    let _: Widget = FilledButton::tonal("Tonal").into();
    let _: Widget = OutlinedButton::new("Outlined").into();
    let _: Widget = TextButton::new("Text").into();
    let _: Widget = IconButton::new("⚙").into();
    let _: Widget = IconButton::icon(SizedBox::square(16.0))
        .tooltip("Settings")
        .into();
    let _: Widget = FloatingActionButton::small(SizedBox::square(20.0)).into();
    let _: Widget = FloatingActionButton::extended("Create").into();
}

#[test]
fn material_default_constraints_follow_tap_target_policy() {
    let theme =
        ThemeData::light().with_tap_target_size(crate::foundation::MaterialTapTargetSize::Padded);
    let style = ButtonKind::Text.default_style();
    let minimum = style.minimum_size.expect("default button minimum");
    assert!(minimum.height >= 36.0);
    assert_eq!(style.tap_target_size, Some(TapTargetSize::Padded));
    assert_eq!(
        theme.material_tap_target_size,
        crate::foundation::MaterialTapTargetSize::Padded
    );
}
