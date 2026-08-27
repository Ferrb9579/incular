//! Bounded lifecycle and theme contracts for the deployable Material P0 set.
//!
//! These tests intentionally construct descriptors through the public facade.
//! They do not create a native window, so they are cheap enough for normal CI
//! while still exercising the retained ownership paths that are most likely to
//! leak (menus, dialogs, tooltips, snackbars, and large ordinary surfaces).

use std::time::{Duration, Instant};

use incular::material::prelude::*;
use incular::prelude::{Color, EdgeInsets, Text, Widget};

#[test]
fn p0_surface_and_control_descriptors_are_repeatable() {
    for index in 0..1_000 {
        let label = format!("Action {index}");
        let _: Widget = ElevatedButton::new(label.clone()).into();
        let _: Widget = FilledButton::tonal(label.clone()).into();
        let _: Widget = OutlinedButton::new(label.clone()).into();
        let _: Widget = TextButton::new(label.clone()).into();
        let _: Widget = IconButton::new("+").tooltip(label.clone()).into();
        let _: Widget = FloatingActionButton::extended(label.clone()).into();
        let _: Widget = Card::new(ListTile::new(Text::new(label.clone())))
            .elevation((index % 12) as f32)
            .margin(EdgeInsets::all(2.0))
            .into();
        let _: Widget = ListTile::new(Text::new(label.clone()))
            .leading(Text::new("•"))
            .trailing(Text::new("›"))
            .on_tap(|| {})
            .into();
        let _: Widget = Checkbox::new(index % 2 == 0).on_changed(|_| {}).into();
        let _: Widget = Switch::new(index % 2 == 0).on_changed(|_| {}).into();
        let _: Widget = Slider::new(0.5)
            .range(0.0, 1.0)
            .divisions(Some(10))
            .on_changed(|_| {})
            .into();
        let _: Widget = RangeSlider::new(RangeValues::new(0.2, 0.8))
            .range(0.0, 1.0)
            .divisions(Some(10))
            .on_changed(|_| {})
            .into();
    }
}

#[test]
fn p0_theme_precedence_and_material_surfaces_remain_sparse() {
    let seed = Color::rgba(0x67, 0x50, 0xa4, 255);
    let theme_override = ComponentThemeData::new()
        .background_color(Color::rgba(10, 20, 30, 255))
        .elevation(7.0);
    let theme = ThemeData::from_seed(seed).copy_with(ThemeDataPatch {
        elevated_button_theme: Some(theme_override.clone()),
        card_theme: Some(ComponentThemeData::new().elevation(9.0)),
        ..ThemeDataPatch::default()
    });
    assert_eq!(theme.elevated_button_theme, theme_override);
    assert_eq!(theme.card_theme.elevation, Some(9.0));
    assert!(ButtonStyle::new().background.is_none());
    assert!(ButtonStyle::new().padding.is_none());

    // The application root must carry the selected theme without copying a
    // second runtime or eagerly materializing the entire tree.
    let app: Widget =
        MaterialApp::new(Scaffold::new(Text::new("body")).app_bar(AppBar::new(Text::new("title"))))
            .theme(theme.clone())
            .dark_theme(ThemeData::dark())
            .theme_mode(ThemeMode::Light)
            .into();
    let _: Widget = Theme::new(theme, app).into();
}

#[test]
fn p0_popup_dialog_tooltip_and_snackbar_lifecycles_are_bounded() {
    let menu = MenuController::new();
    let tooltip = TooltipController::new();
    let dialog = DialogHandle::new(Dialog::new(Text::new("dialog")));
    let messenger = ScaffoldMessengerController::new();

    for _ in 0..1_000 {
        menu.open();
        assert!(menu.is_open());
        menu.close();
        tooltip.show();
        assert!(tooltip.is_visible());
        tooltip.hide();
        dialog.open();
        assert!(dialog.is_open());
        dialog.close();
    }

    for index in 0..1_000 {
        messenger.show_snack_bar(
            SnackBar::text(format!("message {index}")).duration(Duration::from_millis(1)),
        );
    }
    assert!(messenger.is_showing());
    assert_eq!(messenger.queued_count(), 999);

    let mut now = Instant::now() + Duration::from_secs(1);
    let mut removed = 0;
    while messenger.is_showing() {
        if messenger.poll(now) {
            removed += 1;
        } else {
            now += Duration::from_secs(1);
        }
    }
    assert_eq!(removed, 1_000);
    assert_eq!(messenger.queued_count(), 0);
}
