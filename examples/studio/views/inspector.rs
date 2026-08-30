//! Property inspector and document diagnostics pane for Incular Studio.

use super::super::{localization::StudioLocale, state::StudioState, theme::StudioTheme};
use incular::prelude::*;

#[must_use]
pub fn build_inspector(state: &StudioState, theme: &StudioTheme, locale: &StudioLocale) -> Widget {
    let strings = locale.strings();
    let doc = state.active_document();
    let theme_mode = state.theme_mode.get();
    let font_scale = state.font_scale.get();
    let zoom = state.zoom_level.get();

    let header = Container::new().padding(EdgeInsets::all(8.0)).child(
        Text::new(strings.inspector).style(
            TextStyle::new()
                .font_size(11.0)
                .font_weight(FontWeight::BOLD)
                .color(theme.text_muted),
        ),
    );

    let content: Widget = match doc {
        Some(d) => {
            let text = d.controller.text();
            let line_count = text.lines().count();
            let char_count = text.chars().count();
            let byte_count = text.len();
            let is_dirty = d.is_dirty.get();

            let props = Column::new([
                property_row("Document", &d.title, theme),
                property_row("Path", &d.path, theme),
                property_row(
                    "Status",
                    if is_dirty {
                        "Modified (Unsaved)"
                    } else {
                        "Saved"
                    },
                    theme,
                ),
                property_row("Lines", &line_count.to_string(), theme),
                property_row("Characters", &char_count.to_string(), theme),
                property_row("Size", &format!("{byte_count} bytes"), theme),
                property_row("Encoding", "UTF-8", theme),
                property_row("Line Endings", "LF", theme),
                SizedBox::new().height(12.0).into(),
                Text::new("ENVIRONMENT")
                    .style(
                        TextStyle::new()
                            .font_size(10.0)
                            .font_weight(FontWeight::BOLD)
                            .color(theme.text_muted),
                    )
                    .into(),
                SizedBox::new().height(6.0).into(),
                property_row("Theme Mode", &format!("{theme_mode:?}"), theme),
                property_row("Font Scale", &format!("{:.1}x", font_scale), theme),
                property_row("Canvas Zoom", &format!("{:.0}%", zoom * 100.0), theme),
                property_row(
                    "Direction",
                    match locale.direction() {
                        incular_config::TextDirection::Ltr => "Left-to-Right",
                        incular_config::TextDirection::Rtl => "Right-to-Left",
                    },
                    theme,
                ),
            ])
            .spacing(6.0);

            SingleChildScrollView::new(Container::new().padding(EdgeInsets::all(10.0)).child(props))
                .into()
        }
        None => Center::new(
            Text::new("No active selection")
                .style(TextStyle::new().font_size(12.0).color(theme.text_muted)),
        )
        .into(),
    };

    let col_items: Vec<Widget> = vec![header.into(), Expanded::new(content).into()];

    Container::new()
        .color(theme.surface)
        .child(Column::new(col_items))
        .into()
}

fn property_row(label: &str, value: &str, theme: &StudioTheme) -> Widget {
    let label_w: Widget = Text::new(label)
        .style(TextStyle::new().font_size(11.0).color(theme.text_muted))
        .into();
    let value_w: Widget = Text::new(value)
        .style(TextStyle::new().font_size(11.0).color(theme.text_primary))
        .into();
    Row::new([label_w, Spacer::new().into(), value_w]).into()
}
