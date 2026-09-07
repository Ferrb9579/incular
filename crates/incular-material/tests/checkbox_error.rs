use incular_config::Constraints;
use incular_controls::{ControlTheme, ControlThemeScope};
use incular_core::{Color, Size};
use incular_material::{Checkbox, StateProperty, WidgetState};
use incular_rendering::{Brush, PaintCommand};
use incular_widgets::{Border, internal::WidgetTree};

fn indicator(checkbox: Checkbox, theme: ControlTheme) -> (Color, incular_rendering::Border) {
    let mut tree = WidgetTree::new();
    tree.mount(ControlThemeScope::new(theme, checkbox).into())
        .expect("mount");
    tree.layout(Constraints::loose(Size::new(100., 100.)))
        .expect("layout");
    let painted = tree.paint();
    let fill = painted
        .commands()
        .iter()
        .rev()
        .find_map(|command| match command {
            PaintCommand::RRect {
                brush: Brush::Solid(color),
                ..
            } => Some(*color),
            _ => None,
        })
        .expect("indicator fill");
    let border = painted
        .commands()
        .iter()
        .find_map(|command| match command {
            PaintCommand::Border { border, .. } => Some(*border),
            _ => None,
        })
        .expect("indicator border");
    (fill, border)
}

#[test]
fn error_uses_scoped_theme_for_checked_unchecked_and_mixed_indicators() {
    for mut theme in [ControlTheme::light(), ControlTheme::dark()] {
        theme.colors.error = Color::rgba(173, 31, 89, 255);
        for value in [Some(false), Some(true), None] {
            let checkbox = Checkbox::new(false)
                .tristate(true)
                .value(value)
                .is_error(true);
            let (fill, border) = indicator(checkbox, theme.clone());
            assert_eq!(border.color, theme.colors.error, "{value:?}");
            assert_eq!(
                fill,
                if value == Some(false) {
                    theme.colors.surface
                } else {
                    theme.colors.error
                },
                "{value:?}"
            );
        }
    }
}

#[test]
fn checked_value_reaches_the_shared_indicator() {
    let theme = ControlTheme::light();
    let (fill, _) = indicator(Checkbox::new(true), theme.clone());
    assert_eq!(fill, theme.colors.accent);
}

#[test]
fn explicit_fill_and_side_override_error_defaults() {
    let fill = Color::rgba(1, 2, 3, 255);
    let side = Border::new(3., Color::rgba(4, 5, 6, 255));
    let (actual_fill, actual_side) = indicator(
        Checkbox::new(true)
            .is_error(true)
            .fill_color(fill)
            .side(side),
        ControlTheme::light(),
    );
    assert_eq!(actual_fill, fill);
    assert_eq!(actual_side.color, side.top.color);
    assert_eq!(actual_side.width, side.top.width);
}

#[test]
fn disabled_error_preserves_disabled_indicator_colors() {
    for value in [false, true] {
        assert_eq!(
            indicator(
                Checkbox::new(value).enabled(false).is_error(true),
                ControlTheme::light()
            ),
            indicator(Checkbox::new(value).enabled(false), ControlTheme::light()),
        );
    }
}

#[test]
fn disabled_state_reaches_custom_error_color_resolvers() {
    let expected = Color::rgba(10, 20, 30, 255);
    let fill = StateProperty::resolve_with(move |states| {
        assert!(states.contains(WidgetState::Disabled));
        assert!(states.contains(WidgetState::Error));
        assert!(states.contains(WidgetState::Selected));
        expected
    });
    assert_eq!(
        indicator(
            Checkbox::new(true)
                .enabled(false)
                .is_error(true)
                .fill_color(fill),
            ControlTheme::light(),
        )
        .0,
        expected,
    );
}

#[test]
fn checked_and_mixed_values_reach_semantics() {
    use incular_semantics::{CheckedState, Role};

    for (value, expected) in [
        (Some(false), CheckedState::Unchecked),
        (Some(true), CheckedState::Checked),
        (None, CheckedState::Indeterminate),
    ] {
        let mut tree = WidgetTree::new();
        tree.mount(
            Checkbox::new(false)
                .tristate(true)
                .value(value)
                .is_error(true)
                .into(),
        )
        .expect("mount");
        tree.layout(Constraints::loose(Size::new(100., 100.)))
            .expect("layout");
        tree.update_semantics();
        let nodes: Vec<_> = tree
            .semantics()
            .iter()
            .filter(|(_, node)| node.role == Role::Checkbox)
            .collect();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].1.state.checked, Some(expected));
    }
}
