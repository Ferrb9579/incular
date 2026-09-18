use incular_config::{Constraints, EdgeInsets};
use incular_controls::{ControlDensity, ControlState, StateTable, StateValue};
use incular_core::{Color, Size};
use incular_material::{
    ButtonStyleConfig, ComponentThemeData, ElevatedButton, SliderThemeData, Theme, ThemeData,
    ThemeDataPatch, VisualDensity,
};
use incular_widgets::internal::WidgetTree;

#[test]
fn state_table_conversion_preserves_every_control_state() {
    let normal = Color::rgba(1, 1, 1, 255);
    let invalid = Color::rgba(2, 2, 2, 255);
    let expanded = Color::rgba(3, 3, 3, 255);
    let open = Color::rgba(4, 4, 4, 255);
    let read_only = Color::rgba(5, 5, 5, 255);
    let dragging = Color::rgba(6, 6, 6, 255);
    let focus_visible = Color::rgba(7, 7, 7, 255);
    let style = ElevatedButton::style_from(
        ButtonStyleConfig::new().background_color(StateValue::states(
            StateTable::new(normal)
                .invalid(invalid)
                .expanded(expanded)
                .open(open)
                .read_only(read_only)
                .dragging(dragging)
                .focus_visible(focus_visible),
        )),
    );
    let theme = ThemeData::light().control_theme();

    for (state, expected) in [
        (ControlState::INVALID, invalid),
        (ControlState::EXPANDED, expanded),
        (ControlState::OPEN, open),
        (ControlState::READ_ONLY, read_only),
        (ControlState::DRAGGING, dragging),
        (ControlState::FOCUS_VISIBLE, focus_visible),
    ] {
        assert_eq!(style.resolve_background(state, &theme), expected);
    }
}

#[test]
fn component_padding_survives_constructor_defaults() {
    fn width(theme: ThemeData) -> f32 {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Theme::new(theme, ElevatedButton::new("X")).into())
            .expect("mount material button");
        tree.layout(Constraints::loose(Size::new(500.0, 120.0)))
            .expect("layout material button");
        tree.render_size(tree.render_id(root).expect("render id"))
            .expect("render size")
            .width
    }

    let base = width(ThemeData::light());
    let themed = width(ThemeData::light().copy_with(ThemeDataPatch {
        elevated_button_theme: Some(ComponentThemeData::new().padding(EdgeInsets::all(60.0))),
        ..ThemeDataPatch::default()
    }));
    assert!(themed > base + 60.0, "base={base}, themed={themed}");
}

#[test]
fn component_selection_colors_do_not_leak_into_global_control_palette() {
    let base = ThemeData::light();
    let base_controls = base.control_theme();
    let checkbox_color = Color::rgba(240, 10, 20, 255);
    let slider_color = Color::rgba(10, 240, 20, 255);
    let patched = base.copy_with(ThemeDataPatch {
        checkbox_theme: Some(ComponentThemeData::new().fill_color(checkbox_color)),
        slider_theme: Some(SliderThemeData::new().active_track_color(slider_color)),
        ..ThemeDataPatch::default()
    });
    let controls = patched.control_theme();

    assert_eq!(controls.colors.accent, base_controls.colors.accent);
    assert_eq!(
        controls.colors.border_strong,
        base_controls.colors.border_strong
    );
    assert_eq!(
        controls.colors.disabled_foreground,
        base_controls.colors.disabled_foreground
    );
}

#[test]
fn material_visual_density_updates_control_metrics_consistently() {
    let controls = ThemeData::light()
        .with_visual_density(VisualDensity::COMPACT)
        .control_theme();
    assert_eq!(controls.density, ControlDensity::Compact);
    assert_eq!(
        controls.metrics.control_height,
        ControlDensity::Compact.control_height()
    );
    assert_eq!(
        controls.button.height,
        ControlDensity::Compact.control_height()
    );
    assert_eq!(
        controls.input.height,
        ControlDensity::Compact.control_height()
    );
}
