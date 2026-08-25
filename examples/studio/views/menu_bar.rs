//! Top menu and title bar for Incular Studio.

use crate::{
    localization::StudioLocale,
    state::StudioState,
    theme::{StudioTheme, ThemeMode},
    views::components::studio_button,
};
use incular::prelude::*;

#[must_use]
pub fn build_menu_bar(state: &StudioState, theme: &StudioTheme, locale: &StudioLocale) -> Widget {
    let strings = locale.strings();
    let theme_mode = state.theme_mode.get();

    let sidebar_visible = state.sidebar_visible.clone();
    let is_sidebar_open = sidebar_visible.get();
    let inspector_visible = state.inspector_visible.clone();
    let is_inspector_open = inspector_visible.get();
    let bottom_panel_visible = state.bottom_panel_visible.clone();
    let is_bottom_open = bottom_panel_visible.get();
    let command_palette_open = state.command_palette_open.clone();
    let settings_open = state.settings_open.clone();
    let theme_mode_sig = state.theme_mode.clone();
    let locale_sig = state.locale.clone();

    let title_text = Text::new(strings.app_title).style(
        TextStyle::new()
            .font_size(14.0)
            .font_weight(FontWeight::BOLD)
            .color(theme.text_primary),
    );

    // Command palette button
    let palette_btn = MaterialButton::with_child(
        Container::new()
            .padding(EdgeInsets::symmetric(8.0, 4.0))
            .decoration(
                BoxDecoration::new()
                    .color(theme.surface_elevated)
                    .border_radius(BorderRadius::all(Radius::circular(4.0))),
            )
            .child(
                Text::new(format!("🔍 {} (Ctrl+Shift+P)", strings.command_palette))
                    .style(TextStyle::new().font_size(12.0).color(theme.text_muted)),
            ),
    )
    .on_click(move || {
        command_palette_open.set(!command_palette_open.get());
    });

    // Theme toggle button
    let theme_btn = studio_button(
        match theme_mode {
            ThemeMode::Dark => "🌙 Dark",
            ThemeMode::Light => "☀️ Light",
            ThemeMode::HighContrast => "⬛ Contrast",
        },
        theme,
        move || {
            let next = match theme_mode_sig.get() {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::HighContrast,
                ThemeMode::HighContrast => ThemeMode::Dark,
            };
            theme_mode_sig.set(next);
        },
    );

    // Locale toggle button
    let current_locale = locale_sig.get();
    let locale_btn = studio_button(current_locale.display_name(), theme, move || {
        let next = match locale_sig.get() {
            StudioLocale::English => StudioLocale::Arabic,
            StudioLocale::Arabic => StudioLocale::Hindi,
            StudioLocale::Hindi => StudioLocale::Japanese,
            StudioLocale::Japanese => StudioLocale::English,
        };
        locale_sig.set(next);
    });

    // Settings toggle
    let is_settings_open = settings_open.get();
    let settings_btn = studio_button(
        if is_settings_open {
            "⚙️ Close Settings"
        } else {
            "⚙️ Settings"
        },
        theme,
        move || {
            settings_open.set(!settings_open.get());
        },
    );

    // Panel toggles
    let toggle_side_btn = studio_button(
        if is_sidebar_open {
            "◀ Sidebar"
        } else {
            "▶ Sidebar"
        },
        theme,
        move || {
            sidebar_visible.set(!sidebar_visible.get());
        },
    );

    let toggle_bottom_btn = studio_button(
        if is_bottom_open {
            "▼ Panel"
        } else {
            "▲ Panel"
        },
        theme,
        move || {
            bottom_panel_visible.set(!bottom_panel_visible.get());
        },
    );

    let toggle_insp_btn = studio_button(
        if is_inspector_open {
            "Inspector ▶"
        } else {
            "Inspector ◀"
        },
        theme,
        move || {
            inspector_visible.set(!inspector_visible.get());
        },
    );

    let items: Vec<Widget> = vec![
        toggle_side_btn,
        SizedBox::new().width(8.0).into(),
        title_text.into(),
        SizedBox::new().width(16.0).into(),
        palette_btn.into(),
        Spacer::new().into(),
        locale_btn,
        SizedBox::new().width(8.0).into(),
        theme_btn,
        SizedBox::new().width(8.0).into(),
        settings_btn,
        SizedBox::new().width(8.0).into(),
        toggle_bottom_btn,
        SizedBox::new().width(8.0).into(),
        toggle_insp_btn,
    ];

    Container::new()
        .height(36.0)
        .padding(EdgeInsets::symmetric(12.0, 0.0))
        .decoration(
            BoxDecoration::new()
                .color(theme.surface)
                .border(Border::new(1.0, theme.border)),
        )
        .child(Row::new(items).alignment(CrossAxisAlignment::Center))
        .into()
}
