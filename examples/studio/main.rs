//! Incular Studio — Production Reference Application
//! Demonstrates desktop IDE workspace, resizable split panes, virtualized project tree,
//! multi-document text editor, command palette, async search, typed settings forms,
//! multi-lingual localization (including Arabic RTL), theme switching, and state restoration.

mod commands;
mod localization;
mod restoration;
mod state;
mod theme;
mod views;

use self::{
    restoration::RestorationData,
    state::StudioState,
    theme::StudioTheme,
    views::{
        bottom_panel::build_bottom_panel, command_palette::build_command_palette,
        editor::build_editor_workspace, inspector::build_inspector, menu_bar::build_menu_bar,
        project_tree::build_project_tree, settings::build_settings_view,
        status_bar::build_status_bar,
    },
};
use incular::prelude::*;
use incular::widgets::internal::SplitView;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

fn main() {
    let stress_tree_count = std::env::var("INCULAR_STUDIO_STRESS_TREE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());

    let restoration = RestorationData::load_or_default();
    let state = StudioState::new(&restoration, stress_tree_count);

    let state_for_builder = state.clone();
    let app = Application::new(move |_cx| build_studio_app(&state_for_builder))
        .expect("valid Incular Studio application");

    eprintln!("Launching Incular Studio...");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    if let Err(err) = incular::run(app) {
        eprintln!("Incular Studio runtime error: {err:?}");
    }
}

#[must_use]
pub fn build_studio_app(state: &StudioState) -> Widget {
    let theme_mode = state.theme_mode.get();
    let theme = StudioTheme::for_mode(theme_mode);
    let locale = state.locale.get();
    let is_settings_open = state.settings_open.get();
    let is_command_palette_open = state.command_palette_open.get();

    let sidebar_visible = state.sidebar_visible.get();
    let inspector_visible = state.inspector_visible.get();
    let bottom_panel_visible = state.bottom_panel_visible.get();
    let sidebar_width = state.sidebar_width.get();
    let inspector_width = state.inspector_width.get();
    let bottom_height = state.bottom_panel_height.get();

    let sidebar_width_sig = state.sidebar_width.clone();
    let inspector_width_sig = state.inspector_width.clone();
    let bottom_height_sig = state.bottom_panel_height.clone();

    // 1. Menu Bar
    let menu_bar = build_menu_bar(state, &theme, &locale);

    // 2. Main Workspace Layout
    let workspace_content: Widget = if is_settings_open {
        build_settings_view(state, &theme, &locale)
    } else {
        // Editor + Inspector
        let editor_pane = build_editor_workspace(state, &theme, &locale);
        let editor_and_inspector: Widget = if inspector_visible {
            SplitView::horizontal(editor_pane, build_inspector(state, &theme, &locale))
                .second_extent(inspector_width)
                .on_split_changed(move |delta| {
                    inspector_width_sig
                        .set((inspector_width_sig.get() - delta).clamp(180.0, 500.0));
                })
                .into()
        } else {
            editor_pane
        };

        // Top Area (Tree + Editor/Inspector)
        let top_work_area: Widget = if sidebar_visible {
            SplitView::horizontal(
                build_project_tree(state, &theme, &locale),
                editor_and_inspector,
            )
            .first_extent(sidebar_width)
            .on_split_changed(move |delta| {
                sidebar_width_sig.set((sidebar_width_sig.get() + delta).clamp(160.0, 600.0));
            })
            .into()
        } else {
            editor_and_inspector
        };

        // Top + Bottom Split
        if bottom_panel_visible {
            SplitView::vertical(top_work_area, build_bottom_panel(state, &theme, &locale))
                .second_extent(bottom_height)
                .on_split_changed(move |delta| {
                    bottom_height_sig.set((bottom_height_sig.get() - delta).clamp(120.0, 600.0));
                })
                .into()
        } else {
            top_work_area
        }
    };

    // 3. Status Bar
    let status_bar = build_status_bar(state, &theme, &locale);

    let main_scaffold = Container::new().color(theme.background).child(Column::new([
        menu_bar,
        Expanded::new(workspace_content).into(),
        status_bar,
    ]));

    let root_tree: Widget = if is_command_palette_open {
        Stack::new([main_scaffold.into(), build_command_palette(state, &theme)]).into()
    } else {
        main_scaffold.into()
    };

    Directionality::new(locale.direction(), root_tree).into()
}
