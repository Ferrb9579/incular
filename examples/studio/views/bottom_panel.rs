//! Dockable bottom panel for Incular Studio containing Search, Problems, Output, and Canvas.

use super::super::{
    localization::StudioLocale,
    state::{BottomTab, ProblemSeverity, SearchResult, StudioState},
    theme::StudioTheme,
    views::interactive_canvas::build_interactive_canvas,
};
use incular::controls::TextField;
use incular::material::MaterialButton;
use incular::prelude::*;

#[must_use]
pub fn build_bottom_panel(
    state: &StudioState,
    theme: &StudioTheme,
    locale: &StudioLocale,
) -> Widget {
    let strings = locale.strings();
    let current_tab = state.bottom_tab.get();
    let bottom_tab_sig = state.bottom_tab.clone();

    // Tab Headers
    let make_tab_btn = |tab: BottomTab, label: &str| {
        let is_active = current_tab == tab;
        let tab_sig = bottom_tab_sig.clone();
        MaterialButton::with_child(
            Container::new()
                .padding(EdgeInsets::symmetric(10.0, 4.0))
                .decoration(
                    BoxDecoration::new()
                        .color(if is_active {
                            theme.surface_elevated
                        } else {
                            Color::TRANSPARENT
                        })
                        .border_radius(BorderRadius::all(Radius::circular(4.0))),
                )
                .child(
                    Text::new(label).style(
                        TextStyle::new()
                            .font_size(11.0)
                            .font_weight(if is_active {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .color(if is_active {
                                theme.accent
                            } else {
                                theme.text_secondary
                            }),
                    ),
                ),
        )
        .on_click(move || {
            tab_sig.set(tab);
        })
    };

    let problems_count = state.problems.get().len();
    let header_tabs: Vec<Widget> = vec![
        make_tab_btn(BottomTab::Search, strings.search).into(),
        SizedBox::new().width(4.0).into(),
        make_tab_btn(
            BottomTab::Problems,
            &format!("{} ({problems_count})", strings.problems),
        )
        .into(),
        SizedBox::new().width(4.0).into(),
        make_tab_btn(BottomTab::Output, strings.output).into(),
        SizedBox::new().width(4.0).into(),
        make_tab_btn(BottomTab::Canvas, strings.canvas_preview).into(),
    ];
    let header_bar = Container::new()
        .height(30.0)
        .padding(EdgeInsets::symmetric(6.0, 0.0))
        .decoration(
            BoxDecoration::new()
                .color(theme.surface)
                .border(Border::new(1.0, theme.border)),
        )
        .child(Row::new(header_tabs).alignment(CrossAxisAlignment::Center));

    // Tab Contents
    let body: Widget = match current_tab {
        BottomTab::Search => build_search_view(state, theme, strings.search_placeholder),
        BottomTab::Problems => build_problems_view(state, theme),
        BottomTab::Output => build_output_view(state, theme),
        BottomTab::Canvas => build_interactive_canvas(state, theme),
    };

    let main_items: Vec<Widget> = vec![header_bar.into(), Expanded::new(body).into()];

    Container::new()
        .color(theme.background)
        .child(Column::new(main_items))
        .into()
}

fn build_search_view(state: &StudioState, theme: &StudioTheme, placeholder: &str) -> Widget {
    let query = state.search.query.get();
    let results = state.search.results.get();
    let query_sig = state.search.query.clone();
    let results_sig = state.search.results.clone();
    let docs = state.editor.documents.get();

    let search_row_items: Vec<Widget> = vec![
        Text::new("🔍")
            .style(TextStyle::new().font_size(12.0))
            .into(),
        SizedBox::new().width(6.0).into(),
        Expanded::new(
            TextField::new(state.search.controller.clone())
                .placeholder(placeholder)
                .on_submit(move |new_q| {
                    query_sig.set(new_q.clone());
                    let lower = new_q.to_lowercase();
                    let mut found = Vec::new();
                    if !lower.is_empty() {
                        for doc in &docs {
                            let text = doc.controller.text();
                            for (line_idx, line) in text.lines().enumerate() {
                                if line.to_lowercase().contains(&lower) {
                                    found.push(SearchResult {
                                        file: doc.title.clone(),
                                        line_number: line_idx + 1,
                                        match_text: new_q.clone(),
                                        preview: line.trim().to_owned(),
                                    });
                                }
                            }
                        }
                    }
                    results_sig.set(found);
                }),
        )
        .into(),
    ];

    let search_input = Container::new()
        .padding(EdgeInsets::symmetric(8.0, 4.0))
        .decoration(
            BoxDecoration::new()
                .color(theme.surface_elevated)
                .border(Border::new(1.0, theme.border))
                .border_radius(BorderRadius::all(Radius::circular(4.0))),
        )
        .child(Row::new(search_row_items).alignment(CrossAxisAlignment::Center));

    let total_results = results.len();
    let results_list: Widget = if total_results == 0 {
        Center::new(
            Text::new(if query.is_empty() {
                "Enter search term above"
            } else {
                "No matches found"
            })
            .style(TextStyle::new().font_size(12.0).color(theme.text_muted)),
        )
        .into()
    } else {
        let theme_clone = theme.clone();
        ListView::builder(total_results, move |idx| -> Widget {
            let res = &results[idx];
            let res_row_items: Vec<Widget> = vec![
                Text::new(format!("{}:{}", res.file, res.line_number))
                    .style(
                        TextStyle::new()
                            .font_size(11.0)
                            .font_weight(FontWeight::BOLD)
                            .color(theme_clone.accent),
                    )
                    .into(),
                SizedBox::new().width(12.0).into(),
                Expanded::new(
                    Text::new(&res.preview).style(
                        TextStyle::new()
                            .font_size(11.0)
                            .color(theme_clone.text_primary),
                    ),
                )
                .into(),
            ];

            Container::new()
                .padding(EdgeInsets::symmetric(8.0, 4.0))
                .decoration(BoxDecoration::new().border(Border::new(1.0, theme_clone.divider)))
                .child(Row::new(res_row_items).alignment(CrossAxisAlignment::Center))
                .into()
        })
        .into()
    };

    let search_panel_items: Vec<Widget> = vec![
        search_input.into(),
        SizedBox::new().height(6.0).into(),
        Expanded::new(results_list).into(),
    ];

    Container::new()
        .padding(EdgeInsets::all(8.0))
        .child(Column::new(search_panel_items))
        .into()
}

fn build_problems_view(state: &StudioState, theme: &StudioTheme) -> Widget {
    let problems = state.problems.get();
    if problems.is_empty() {
        return Center::new(
            Text::new("No problems detected in workspace.")
                .style(TextStyle::new().font_size(12.0).color(theme.text_muted)),
        )
        .into();
    }

    let theme_clone = theme.clone();
    ListView::builder(problems.len(), move |idx| -> Widget {
        let prob = &problems[idx];
        let (icon, color) = match prob.severity {
            ProblemSeverity::Error => ("❌", theme_clone.error),
            ProblemSeverity::Warning => ("⚠️", theme_clone.warning),
            ProblemSeverity::Info => ("ℹ️", theme_clone.info),
        };

        let prob_items: Vec<Widget> = vec![
            Text::new(icon)
                .style(TextStyle::new().font_size(11.0))
                .into(),
            SizedBox::new().width(6.0).into(),
            Text::new(&prob.message)
                .style(TextStyle::new().font_size(11.0).color(color))
                .into(),
            Spacer::new().into(),
            Text::new(format!("{}:{}", prob.file, prob.line))
                .style(
                    TextStyle::new()
                        .font_size(11.0)
                        .color(theme_clone.text_muted),
                )
                .into(),
        ];

        Container::new()
            .padding(EdgeInsets::symmetric(8.0, 4.0))
            .child(Row::new(prob_items).alignment(CrossAxisAlignment::Center))
            .into()
    })
    .into()
}

fn build_output_view(state: &StudioState, theme: &StudioTheme) -> Widget {
    let lines = state.output_lines.get();
    let text = lines.join("\n");

    SingleChildScrollView::new(
        Container::new().padding(EdgeInsets::all(8.0)).child(
            Text::new(text).style(
                TextStyle::new()
                    .font_size(11.0)
                    .line_height(Some(1.4))
                    .color(theme.text_secondary),
            ),
        ),
    )
    .into()
}
