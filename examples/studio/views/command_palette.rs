//! Command palette modal overlay for Incular Studio.
//! Provides fast command execution and keyboard navigation.

use crate::{
    commands::StudioCommand,
    state::StudioState,
    theme::{StudioTheme, ThemeMode},
};
use incular::controls::TextField;
use incular::material::MaterialButton;
use incular::prelude::*;

#[must_use]
pub fn build_command_palette(state: &StudioState, theme: &StudioTheme) -> Widget {
    let all_commands = vec![
        StudioCommand::NewFile,
        StudioCommand::OpenFile,
        StudioCommand::SaveFile,
        StudioCommand::CloseTab,
        StudioCommand::ToggleSidebar,
        StudioCommand::ToggleInspector,
        StudioCommand::ToggleBottomPanel,
        StudioCommand::OpenSettings,
        StudioCommand::ToggleTheme,
        StudioCommand::ToggleWrap,
        StudioCommand::ZoomIn,
        StudioCommand::ZoomOut,
        StudioCommand::ResetZoom,
        StudioCommand::FindInWorkspace,
    ];

    let open_sig = state.command_palette_open.clone();
    let theme_mode_sig = state.theme_mode.clone();
    let sidebar_sig = state.sidebar_visible.clone();
    let inspector_sig = state.inspector_visible.clone();
    let bottom_sig = state.bottom_panel_visible.clone();
    let settings_sig = state.settings_open.clone();
    let zoom_sig = state.zoom_level.clone();
    let wrap_sig = state.settings.word_wrap.clone();

    let mut command_items: Vec<Widget> = Vec::new();
    for cmd in all_commands {
        let title = cmd.title();
        let shortcut = cmd.shortcut_display();
        let open_sig_item = open_sig.clone();
        let theme_sig_item = theme_mode_sig.clone();
        let side_sig_item = sidebar_sig.clone();
        let insp_sig_item = inspector_sig.clone();
        let bot_sig_item = bottom_sig.clone();
        let set_sig_item = settings_sig.clone();
        let zm_sig_item = zoom_sig.clone();
        let wr_sig_item = wrap_sig.clone();

        let cmd_row: Vec<Widget> = vec![
            Text::new(title)
                .style(TextStyle::new().font_size(12.0).color(theme.text_primary))
                .into(),
            Spacer::new().into(),
            Text::new(shortcut)
                .style(TextStyle::new().font_size(10.0).color(theme.text_muted))
                .into(),
        ];

        let item = MaterialButton::with_child(
            Container::new()
                .padding(EdgeInsets::symmetric(12.0, 8.0))
                .decoration(
                    BoxDecoration::new()
                        .color(theme.surface)
                        .border(Border::new(1.0, theme.divider)),
                )
                .child(Row::new(cmd_row).alignment(CrossAxisAlignment::Center)),
        )
        .on_click(move || {
            open_sig_item.set(false);
            match cmd {
                StudioCommand::ToggleTheme => {
                    let next = match theme_sig_item.get() {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::HighContrast,
                        ThemeMode::HighContrast => ThemeMode::Dark,
                    };
                    theme_sig_item.set(next);
                }
                StudioCommand::ToggleSidebar => {
                    side_sig_item.set(!side_sig_item.get());
                }
                StudioCommand::ToggleInspector => {
                    insp_sig_item.set(!insp_sig_item.get());
                }
                StudioCommand::ToggleBottomPanel => {
                    bot_sig_item.set(!bot_sig_item.get());
                }
                StudioCommand::OpenSettings => {
                    set_sig_item.set(true);
                }
                StudioCommand::ToggleWrap => {
                    wr_sig_item.set(!wr_sig_item.get());
                }
                StudioCommand::ZoomIn => {
                    zm_sig_item.set((zm_sig_item.get() * 1.2).min(4.0));
                }
                StudioCommand::ZoomOut => {
                    zm_sig_item.set((zm_sig_item.get() / 1.2).max(0.25));
                }
                StudioCommand::ResetZoom => {
                    zm_sig_item.set(1.0);
                }
                _ => {}
            }
        });
        command_items.push(item.into());
    }

    let box_items: Vec<Widget> = vec![
        Container::new()
            .padding(EdgeInsets::symmetric(10.0, 6.0))
            .decoration(
                BoxDecoration::new()
                    .color(theme.surface)
                    .border_radius(BorderRadius::all(Radius::circular(4.0))),
            )
            .child(
                TextField::new(state.command_palette_controller.clone())
                    .placeholder("Type a command or search..."),
            )
            .into(),
        SizedBox::new().height(8.0).into(),
        Column::new(command_items).into(),
    ];

    let palette_box = Container::new()
        .width(480.0)
        .padding(EdgeInsets::all(8.0))
        .decoration(
            BoxDecoration::new()
                .color(theme.surface_elevated)
                .border(Border::new(1.0, theme.border_focus))
                .border_radius(BorderRadius::all(Radius::circular(8.0)))
                .box_shadow(vec![BoxShadow::new(
                    Color::rgba(0, 0, 0, 160),
                    Offset::new(0.0, 8.0),
                    24.0,
                    0.0,
                )]),
        )
        .child(Column::new(box_items));

    let dismiss_sig = state.command_palette_open.clone();
    let backdrop =
        GestureDetector::new(Container::new().color(Color::rgba(0, 0, 0, 100))).on_tap(move || {
            dismiss_sig.set(false);
        });

    let stack_items: Vec<Widget> = vec![
        backdrop.into(),
        Positioned::new(Center::new(palette_box))
            .top(60.0)
            .left(0.0)
            .right(0.0)
            .into(),
    ];

    Stack::new(stack_items).into()
}
