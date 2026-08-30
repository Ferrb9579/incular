//! Settings and configuration form view for Incular Studio.
//! Uses typed Form, validation rules, and reactive state bindings.

use super::super::{
    localization::StudioLocale,
    state::StudioState,
    theme::{StudioTheme, ThemeMode},
    views::components::studio_button,
};
use incular::material::MaterialButton;
use incular::prelude::*;

#[must_use]
pub fn build_settings_view(
    state: &StudioState,
    theme: &StudioTheme,
    locale: &StudioLocale,
) -> Widget {
    let strings = locale.strings();
    let close_sig = state.settings_open.clone();

    let font_size = state.settings.editor_font_size.get();
    let tab_size = state.settings.tab_size.get();
    let word_wrap = state.settings.word_wrap.get();
    let auto_save = state.settings.auto_save.get();
    let current_theme = state.theme_mode.get();
    let current_locale = state.locale.get();

    let font_size_sig = state.settings.editor_font_size.clone();
    let tab_size_sig = state.settings.tab_size.clone();
    let word_wrap_sig = state.settings.word_wrap.clone();
    let auto_save_sig = state.settings.auto_save.clone();
    let theme_sig = state.theme_mode.clone();
    let locale_sig = state.locale.clone();

    let header_items: Vec<Widget> = vec![
        Text::new(strings.settings)
            .style(
                TextStyle::new()
                    .font_size(16.0)
                    .font_weight(FontWeight::BOLD)
                    .color(theme.text_primary),
            )
            .into(),
        Spacer::new().into(),
        studio_button("✕ Close", theme, move || {
            close_sig.set(false);
        }),
    ];
    let header = Row::new(header_items).alignment(CrossAxisAlignment::Center);

    // Font size controls
    let font_inc_sig = font_size_sig.clone();
    let font_dec_sig = font_size_sig.clone();
    let font_row_items: Vec<Widget> = vec![
        Text::new(strings.font_size)
            .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
            .into(),
        Spacer::new().into(),
        studio_button(" - ", theme, move || {
            font_dec_sig.set((font_dec_sig.get() - 1.0).max(8.0));
        }),
        SizedBox::new().width(8.0).into(),
        Text::new(format!("{font_size:.0} pt"))
            .style(TextStyle::new().font_size(12.0).color(theme.accent))
            .into(),
        SizedBox::new().width(8.0).into(),
        studio_button(" + ", theme, move || {
            font_inc_sig.set((font_inc_sig.get() + 1.0).min(32.0));
        }),
    ];
    let font_row = Row::new(font_row_items).alignment(CrossAxisAlignment::Center);

    // Tab size controls
    let tab_2_sig = tab_size_sig.clone();
    let tab_4_sig = tab_size_sig.clone();
    let tab_8_sig = tab_size_sig.clone();
    let tab_row_items: Vec<Widget> = vec![
        Text::new(strings.tab_size)
            .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
            .into(),
        Spacer::new().into(),
        studio_button(
            if tab_size == 2 { "[2]" } else { " 2 " },
            theme,
            move || {
                tab_2_sig.set(2);
            },
        ),
        SizedBox::new().width(4.0).into(),
        studio_button(
            if tab_size == 4 { "[4]" } else { " 4 " },
            theme,
            move || {
                tab_4_sig.set(4);
            },
        ),
        SizedBox::new().width(4.0).into(),
        studio_button(
            if tab_size == 8 { "[8]" } else { " 8 " },
            theme,
            move || {
                tab_8_sig.set(8);
            },
        ),
    ];
    let tab_row = Row::new(tab_row_items).alignment(CrossAxisAlignment::Center);

    // Word wrap toggle
    let wrap_toggle_sig = word_wrap_sig.clone();
    let wrap_row_items: Vec<Widget> = vec![
        Text::new(strings.word_wrap)
            .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
            .into(),
        Spacer::new().into(),
        studio_button(
            if word_wrap {
                "✅ Enabled"
            } else {
                "⬜ Disabled"
            },
            theme,
            move || {
                wrap_toggle_sig.set(!wrap_toggle_sig.get());
            },
        ),
    ];
    let wrap_row = Row::new(wrap_row_items).alignment(CrossAxisAlignment::Center);

    // Auto save toggle
    let auto_toggle_sig = auto_save_sig.clone();
    let auto_row_items: Vec<Widget> = vec![
        Text::new(strings.auto_save)
            .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
            .into(),
        Spacer::new().into(),
        studio_button(
            if auto_save {
                "✅ Enabled"
            } else {
                "⬜ Disabled"
            },
            theme,
            move || {
                auto_toggle_sig.set(!auto_toggle_sig.get());
            },
        ),
    ];
    let auto_row = Row::new(auto_row_items).alignment(CrossAxisAlignment::Center);

    // Theme selector
    let theme_dark_sig = theme_sig.clone();
    let theme_light_sig = theme_sig.clone();
    let theme_hc_sig = theme_sig.clone();
    let theme_row_items: Vec<Widget> = vec![
        Text::new(strings.theme)
            .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
            .into(),
        Spacer::new().into(),
        studio_button(
            if current_theme == ThemeMode::Dark {
                "● Dark"
            } else {
                "○ Dark"
            },
            theme,
            move || {
                theme_dark_sig.set(ThemeMode::Dark);
            },
        ),
        SizedBox::new().width(6.0).into(),
        studio_button(
            if current_theme == ThemeMode::Light {
                "● Light"
            } else {
                "○ Light"
            },
            theme,
            move || {
                theme_light_sig.set(ThemeMode::Light);
            },
        ),
        SizedBox::new().width(6.0).into(),
        studio_button(
            if current_theme == ThemeMode::HighContrast {
                "● Contrast"
            } else {
                "○ Contrast"
            },
            theme,
            move || {
                theme_hc_sig.set(ThemeMode::HighContrast);
            },
        ),
    ];
    let theme_row = Row::new(theme_row_items).alignment(CrossAxisAlignment::Center);

    // Language selector
    let loc_en_sig = locale_sig.clone();
    let loc_ar_sig = locale_sig.clone();
    let loc_hi_sig = locale_sig.clone();
    let loc_ja_sig = locale_sig.clone();
    let lang_row_items: Vec<Widget> = vec![
        Text::new(strings.language)
            .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
            .into(),
        Spacer::new().into(),
        studio_button(
            if current_locale == StudioLocale::English {
                "● English"
            } else {
                "○ English"
            },
            theme,
            move || {
                loc_en_sig.set(StudioLocale::English);
            },
        ),
        SizedBox::new().width(4.0).into(),
        studio_button(
            if current_locale == StudioLocale::Arabic {
                "● العربية"
            } else {
                "○ العربية"
            },
            theme,
            move || {
                loc_ar_sig.set(StudioLocale::Arabic);
            },
        ),
        SizedBox::new().width(4.0).into(),
        studio_button(
            if current_locale == StudioLocale::Hindi {
                "● हिन्दी"
            } else {
                "○ हिन्दी"
            },
            theme,
            move || {
                loc_hi_sig.set(StudioLocale::Hindi);
            },
        ),
        SizedBox::new().width(4.0).into(),
        studio_button(
            if current_locale == StudioLocale::Japanese {
                "● 日本語"
            } else {
                "○ 日本語"
            },
            theme,
            move || {
                loc_ja_sig.set(StudioLocale::Japanese);
            },
        ),
    ];
    let lang_row = Row::new(lang_row_items).alignment(CrossAxisAlignment::Center);

    // Reset Defaults button
    let reset_font_sig = font_size_sig.clone();
    let reset_tab_sig = tab_size_sig.clone();
    let reset_wrap_sig = word_wrap_sig.clone();
    let reset_auto_sig = auto_save_sig.clone();
    let reset_theme_sig = theme_sig.clone();
    let reset_loc_sig = locale_sig.clone();
    let reset_btn = MaterialButton::with_child(
        Container::new()
            .padding(EdgeInsets::symmetric(12.0, 8.0))
            .decoration(
                BoxDecoration::new()
                    .color(theme.surface_elevated)
                    .border(Border::new(1.0, theme.border))
                    .border_radius(BorderRadius::all(Radius::circular(4.0))),
            )
            .child(
                Text::new(strings.reset_defaults)
                    .style(TextStyle::new().font_size(12.0).color(theme.warning)),
            ),
    )
    .on_click(move || {
        reset_font_sig.set(14.0);
        reset_tab_sig.set(4);
        reset_wrap_sig.set(false);
        reset_auto_sig.set(true);
        reset_theme_sig.set(ThemeMode::Dark);
        reset_loc_sig.set(StudioLocale::English);
    });

    let form_items: Vec<Widget> = vec![
        header.into(),
        SizedBox::new().height(16.0).into(),
        font_row.into(),
        SizedBox::new().height(12.0).into(),
        tab_row.into(),
        SizedBox::new().height(12.0).into(),
        wrap_row.into(),
        SizedBox::new().height(12.0).into(),
        auto_row.into(),
        SizedBox::new().height(12.0).into(),
        theme_row.into(),
        SizedBox::new().height(12.0).into(),
        lang_row.into(),
        SizedBox::new().height(24.0).into(),
        reset_btn.into(),
    ];
    let form_content = Column::new(form_items);

    Container::new()
        .color(theme.background)
        .padding(EdgeInsets::all(24.0))
        .child(SingleChildScrollView::new(form_content))
        .into()
}
