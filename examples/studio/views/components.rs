//! Reusable styled UI components for Incular Studio.

#![allow(dead_code)]

use crate::theme::StudioTheme;
use incular::prelude::*;
use incular_controls::prelude::{Button as ControlButton, ButtonStyle, GhostButton, PrimaryButton};

/// Creates a standard styled toolbar / panel button.
#[must_use]
pub fn studio_button(
    label: impl Into<String>,
    theme: &StudioTheme,
    on_click: impl Fn() + 'static,
) -> Widget {
    ControlButton::new(label)
        .style(
            ButtonStyle::new()
                .background(theme.surface_elevated)
                .foreground(theme.text_primary)
                .border(Border::new(1.0, theme.border))
                .border_radius(4.0)
                .height(26.0)
                .padding(EdgeInsets::symmetric(8.0, 4.0)),
        )
        .on_click(on_click)
        .into()
}

/// Creates a prominent accent-colored action button.
#[must_use]
pub fn studio_accent_button(
    label: impl Into<String>,
    theme: &StudioTheme,
    on_click: impl Fn() + 'static,
) -> Widget {
    PrimaryButton::new(label)
        .style(
            ButtonStyle::new()
                .background(theme.accent)
                .foreground(Color::WHITE)
                .border_radius(4.0)
                .height(28.0)
                .padding(EdgeInsets::symmetric(12.0, 6.0)),
        )
        .on_click(on_click)
        .into()
}

/// Creates a compact icon button (e.g. close, add, toggle).
#[must_use]
pub fn studio_icon_button(
    icon: impl Into<String>,
    theme: &StudioTheme,
    on_click: impl Fn() + 'static,
) -> Widget {
    GhostButton::new(icon)
        .style(
            ButtonStyle::new()
                .foreground(theme.text_muted)
                .height(22.0)
                .padding(EdgeInsets::symmetric(4.0, 2.0)),
        )
        .on_click(on_click)
        .into()
}

/// Creates a panel section header with standard styling.
#[must_use]
pub fn studio_section_header(title: impl Into<String>, theme: &StudioTheme) -> Widget {
    Container::new()
        .height(28.0)
        .padding(EdgeInsets::symmetric(8.0, 4.0))
        .color(theme.surface_elevated)
        .child(
            Text::new(title).style(
                TextStyle::new()
                    .font_size(11.0)
                    .font_weight(FontWeight::BOLD)
                    .color(theme.text_secondary),
            ),
        )
        .into()
}
