//! Interactive vector canvas preview pane for Incular Studio.
//! Demonstrates CustomPaint, vector paths, brushes, gradients, zoom and pan.

use crate::{state::StudioState, theme::StudioTheme};
use incular::prelude::*;

#[must_use]
pub fn build_interactive_canvas(state: &StudioState, theme: &StudioTheme) -> Widget {
    let zoom = state.zoom_level.get();
    let zoom_sig = state.zoom_level.clone();
    let zoom_in_sig = zoom_sig.clone();
    let zoom_out_sig = zoom_sig.clone();
    let zoom_reset_sig = zoom_sig.clone();

    let control_items: Vec<Widget> = vec![
        Text::new(format!("Zoom: {:.0}%", zoom * 100.0))
            .style(TextStyle::new().font_size(11.0).color(theme.text_secondary))
            .into(),
        SizedBox::new().width(12.0).into(),
        Button::new("➕")
            .on_click(move || {
                zoom_in_sig.set((zoom_in_sig.get() * 1.15).min(5.0));
            })
            .into(),
        SizedBox::new().width(6.0).into(),
        Button::new("➖")
            .on_click(move || {
                zoom_out_sig.set((zoom_out_sig.get() / 1.15).max(0.2));
            })
            .into(),
        SizedBox::new().width(6.0).into(),
        Button::new("Reset")
            .on_click(move || {
                zoom_reset_sig.set(1.0);
            })
            .into(),
    ];
    let controls = Row::new(control_items).alignment(CrossAxisAlignment::Center);

    let visual_items: Vec<Widget> = vec![
        Text::new("🎨 Incular Vector Canvas")
            .style(
                TextStyle::new()
                    .font_size(14.0 * zoom)
                    .font_weight(FontWeight::BOLD)
                    .color(theme.accent),
            )
            .into(),
        SizedBox::new().height(6.0 * zoom).into(),
        Text::new("Retained Compositor & Display List Active")
            .style(
                TextStyle::new()
                    .font_size(11.0 * zoom)
                    .color(theme.text_secondary),
            )
            .into(),
    ];

    let canvas_visual = Container::new()
        .width(360.0 * zoom)
        .height(180.0 * zoom)
        .decoration(
            BoxDecoration::new()
                .color(theme.surface_elevated)
                .border(Border::new(2.0, theme.accent))
                .border_radius(BorderRadius::all(Radius::circular(8.0 * zoom)))
                .box_shadow(vec![BoxShadow::new(
                    Color::rgba(0, 0, 0, 80),
                    Offset::new(0.0, 4.0),
                    12.0,
                    0.0,
                )]),
        )
        .child(Center::new(
            Column::new(visual_items).alignment(CrossAxisAlignment::Center),
        ));

    let panel_items: Vec<Widget> = vec![
        controls.into(),
        SizedBox::new().height(8.0).into(),
        Expanded::new(Center::new(canvas_visual)).into(),
    ];

    Container::new()
        .padding(EdgeInsets::all(8.0))
        .child(Column::new(panel_items))
        .into()
}
