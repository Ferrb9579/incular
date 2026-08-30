//! Bottom status bar for Incular Studio.

use super::super::{localization::StudioLocale, state::StudioState, theme::StudioTheme};
use incular::prelude::*;

#[must_use]
pub fn build_status_bar(state: &StudioState, theme: &StudioTheme, locale: &StudioLocale) -> Widget {
    let strings = locale.strings();
    let status_msg = state.status_message.get();
    let doc = state.active_document();
    let theme_mode = state.theme_mode.get();
    let tab_size = state.settings.tab_size.get();

    let (line, col) = match doc {
        Some(d) => (d.cursor_line.get(), d.cursor_col.get()),
        None => (1, 1),
    };

    let left_items: Vec<Widget> = vec![
        Text::new("⚡")
            .style(TextStyle::new().font_size(11.0))
            .into(),
        SizedBox::new().width(4.0).into(),
        Text::new(&status_msg)
            .style(TextStyle::new().font_size(11.0).color(theme.text_secondary))
            .into(),
    ];
    let status_left: Widget = Row::new(left_items)
        .alignment(CrossAxisAlignment::Center)
        .into();

    let right_items: Vec<Widget> = vec![
        Text::new(format!(
            "{}: {line}, {}: {col}",
            strings.line, strings.column
        ))
        .style(TextStyle::new().font_size(11.0).color(theme.text_secondary))
        .into(),
        SizedBox::new().width(12.0).into(),
        Text::new(format!("Spaces: {tab_size}"))
            .style(TextStyle::new().font_size(11.0).color(theme.text_secondary))
            .into(),
        SizedBox::new().width(12.0).into(),
        Text::new("UTF-8")
            .style(TextStyle::new().font_size(11.0).color(theme.text_secondary))
            .into(),
        SizedBox::new().width(12.0).into(),
        Text::new(locale.code())
            .style(TextStyle::new().font_size(11.0).color(theme.accent))
            .into(),
        SizedBox::new().width(12.0).into(),
        Text::new(format!("{theme_mode:?}"))
            .style(TextStyle::new().font_size(11.0).color(theme.text_muted))
            .into(),
    ];
    let status_right: Widget = Row::new(right_items)
        .alignment(CrossAxisAlignment::Center)
        .into();

    Container::new()
        .height(24.0)
        .padding(EdgeInsets::symmetric(8.0, 0.0))
        .decoration(
            BoxDecoration::new()
                .color(theme.surface)
                .border(Border::new(1.0, theme.border)),
        )
        .child(
            Row::new([status_left, Spacer::new().into(), status_right])
                .alignment(CrossAxisAlignment::Center),
        )
        .into()
}
