//! Document tab bar and text editor workspace for Incular Studio.
//! Uses persistent retained TextEditingController and ScrollController.

use crate::{
    localization::StudioLocale,
    state::{DocumentTab, StudioState},
    theme::StudioTheme,
};
use incular::prelude::*;

#[must_use]
pub fn build_editor_workspace(
    state: &StudioState,
    theme: &StudioTheme,
    locale: &StudioLocale,
) -> Widget {
    let _strings = locale.strings();
    let documents = state.editor.documents.get();
    let active_idx = state.editor.active_tab_index.get();
    let font_size = state.settings.editor_font_size.get();

    // 1. Build Tab Bar
    let mut tab_widgets: Vec<Widget> = Vec::new();
    let docs_sig = state.editor.documents.clone();
    let active_sig = state.editor.active_tab_index.clone();

    for (idx, doc) in documents.iter().enumerate() {
        let is_active = idx == active_idx;
        let is_dirty = doc.is_dirty.get();
        let doc_idx = idx;

        let active_sig_close = active_sig.clone();
        let active_sig_select = active_sig.clone();
        let docs_sig_item = docs_sig.clone();

        let tab_label_items: Vec<Widget> = vec![
            Text::new(&doc.title)
                .style(
                    TextStyle::new()
                        .font_size(12.0)
                        .font_weight(if is_active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .color(if is_active {
                            theme.text_primary
                        } else {
                            theme.text_secondary
                        }),
                )
                .into(),
            SizedBox::new().width(6.0).into(),
            if is_dirty {
                Text::new("●")
                    .style(TextStyle::new().font_size(10.0).color(theme.accent))
                    .into()
            } else {
                SizedBox::new().width(0.0).into()
            },
            SizedBox::new().width(6.0).into(),
            MaterialButton::with_child(
                Text::new("✕").style(TextStyle::new().font_size(10.0).color(theme.text_muted)),
            )
            .on_click(move || {
                let mut docs = docs_sig_item.get();
                if doc_idx < docs.len() {
                    docs.remove(doc_idx);
                    let current_active = active_sig_close.get();
                    if current_active >= docs.len() && !docs.is_empty() {
                        active_sig_close.set(docs.len() - 1);
                    }
                    docs_sig_item.set(docs);
                }
            })
            .into(),
        ];
        let tab_label = Row::new(tab_label_items).alignment(CrossAxisAlignment::Center);

        let tab_container = Container::new()
            .padding(EdgeInsets::symmetric(10.0, 6.0))
            .decoration(
                BoxDecoration::new()
                    .color(if is_active {
                        theme.background
                    } else {
                        theme.surface_elevated
                    })
                    .border(Border::new(
                        1.0,
                        if is_active {
                            theme.accent
                        } else {
                            theme.border
                        },
                    )),
            )
            .child(tab_label);

        let tab_button = MaterialButton::with_child(tab_container).on_click(move || {
            active_sig_select.set(doc_idx);
        });

        tab_widgets.push(tab_button.into());
    }

    // New File button
    let docs_sig_new = docs_sig.clone();
    let active_sig_new = active_sig.clone();
    let new_tab_btn = MaterialButton::with_child(
        Container::new()
            .padding(EdgeInsets::symmetric(8.0, 6.0))
            .child(Text::new("+").style(TextStyle::new().font_size(14.0).color(theme.text_muted))),
    )
    .on_click(move || {
        let mut docs = docs_sig_new.get();
        let id = format!("untitled_{}", docs.len() + 1);
        let title = format!("Untitled-{}", docs.len() + 1);
        docs.push(DocumentTab::new(&id, &title, &id, "// New empty buffer\n"));
        active_sig_new.set(docs.len() - 1);
        docs_sig_new.set(docs);
    });
    tab_widgets.push(new_tab_btn.into());

    let tab_bar = Container::new()
        .height(34.0)
        .padding(EdgeInsets::symmetric(6.0, 0.0))
        .decoration(
            BoxDecoration::new()
                .color(theme.surface)
                .border(Border::new(1.0, theme.border)),
        )
        .child(
            SingleChildScrollView::new(Row::new(tab_widgets).alignment(CrossAxisAlignment::Center))
                .scroll_direction(Axis::Horizontal),
        );

    // 2. Editor Body
    let active_doc = documents.get(active_idx);
    let editor_body: Widget = match active_doc {
        Some(doc) => {
            let line_count = doc.controller.text().lines().count().max(1);
            let line_numbers = (1..=line_count)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("\n");

            let gutter = Container::new()
                .padding(EdgeInsets::symmetric(8.0, 8.0))
                .color(theme.surface_elevated)
                .child(
                    Text::new(line_numbers).style(
                        TextStyle::new()
                            .font_size(font_size)
                            .line_height(Some(1.4))
                            .color(theme.line_number),
                    ),
                );

            let text_editor = Container::new()
                .padding(EdgeInsets::all(8.0))
                .color(theme.background)
                .child(
                    TextField::new(doc.controller.clone())
                        .multiline(true)
                        .style(
                            TextStyle::new()
                                .font_size(font_size)
                                .line_height(Some(1.4))
                                .color(theme.text_primary),
                        ),
                );

            let editor_row_items: Vec<Widget> = vec![
                gutter.into(),
                Expanded::new(
                    SingleChildScrollView::new(text_editor)
                        .controller(doc.scroll_controller.clone()),
                )
                .into(),
            ];

            Row::new(editor_row_items).into()
        }
        None => Center::new(
            Text::new("No documents open. Press Ctrl+N to create a new buffer.")
                .style(TextStyle::new().font_size(13.0).color(theme.text_muted)),
        )
        .into(),
    };

    let workspace_items: Vec<Widget> = vec![tab_bar.into(), Expanded::new(editor_body).into()];

    Container::new()
        .color(theme.background)
        .child(Column::new(workspace_items))
        .into()
}
