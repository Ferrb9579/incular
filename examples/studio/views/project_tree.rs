//! Virtualized file/project tree explorer for Incular Studio.
//! Supports tens of thousands of logical nodes with bounded O(visible) materialization.

use crate::{
    localization::StudioLocale,
    state::{DocumentTab, StudioState},
    theme::StudioTheme,
};
use incular::prelude::*;

#[must_use]
pub fn build_project_tree(
    state: &StudioState,
    theme: &StudioTheme,
    locale: &StudioLocale,
) -> Widget {
    let strings = locale.strings();
    let expanded = state.project.expanded_nodes.get();
    let selected = state.project.selected_node.get();
    let filter = state.project.filter_query.get().to_lowercase();
    let all_nodes = state.project.nodes.clone();

    // Compute visible flattened items based on expansion state and filter
    let mut visible_indices: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < all_nodes.len() {
        let node = &all_nodes[i];
        let matches_filter = filter.is_empty() || node.name.to_lowercase().contains(&filter);

        if matches_filter {
            visible_indices.push(i);
        }

        // If it's a directory and not expanded and no filter active, skip its children
        if node.is_dir && !expanded.contains(&node.id) && filter.is_empty() {
            let current_depth = node.depth;
            i += 1;
            while i < all_nodes.len() && all_nodes[i].depth > current_depth {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let total_visible = visible_indices.len();
    let expanded_sig = state.project.expanded_nodes.clone();
    let selected_sig = state.project.selected_node.clone();
    let documents_sig = state.editor.documents.clone();
    let active_tab_sig = state.editor.active_tab_index.clone();
    let theme_clone = theme.clone();

    let tree_list = ListView::builder(total_visible, move |idx| {
        let node_idx = visible_indices[idx];
        let node = &all_nodes[node_idx];
        let is_expanded = expanded.contains(&node.id);
        let is_selected = selected.as_deref() == Some(&node.id);

        let chevron = if node.is_dir {
            if is_expanded { "▼ " } else { "▶ " }
        } else {
            "  "
        };

        let node_id = node.id.clone();
        let node_name = node.name.clone();
        let node_path = node.path.clone();
        let is_dir = node.is_dir;

        let expanded_sig_item = expanded_sig.clone();
        let selected_sig_item = selected_sig.clone();
        let docs_sig_item = documents_sig.clone();
        let active_sig_item = active_tab_sig.clone();

        let row_items: Vec<Widget> = vec![
            SizedBox::new().width(node.depth as f32 * 14.0).into(),
            Text::new(chevron)
                .style(
                    TextStyle::new()
                        .font_size(10.0)
                        .color(theme_clone.text_muted),
                )
                .into(),
            Text::new(node.icon)
                .style(TextStyle::new().font_size(13.0))
                .into(),
            SizedBox::new().width(6.0).into(),
            Text::new(&node.name)
                .style(TextStyle::new().font_size(13.0).color(if is_selected {
                    theme_clone.accent
                } else {
                    theme_clone.text_primary
                }))
                .into(),
        ];
        let row_content = Row::new(row_items).alignment(CrossAxisAlignment::Center);

        let item_container = Container::new()
            .height(26.0)
            .padding(EdgeInsets::symmetric(8.0, 2.0))
            .decoration(
                BoxDecoration::new()
                    .color(if is_selected {
                        theme_clone.surface_active
                    } else {
                        Color::TRANSPARENT
                    })
                    .border_radius(BorderRadius::all(Radius::circular(4.0))),
            )
            .child(row_content);

        let btn: Widget = Button::with_child(item_container)
            .on_click(move || {
                selected_sig_item.set(Some(node_id.clone()));
                if is_dir {
                    let mut exp = expanded_sig_item.get();
                    if exp.contains(&node_id) {
                        exp.remove(&node_id);
                    } else {
                        exp.insert(node_id.clone());
                    }
                    expanded_sig_item.set(exp);
                } else {
                    // Open in editor
                    let mut docs = docs_sig_item.get();
                    if let Some(pos) = docs.iter().position(|d| d.path == node_path) {
                        active_sig_item.set(pos);
                    } else {
                        let new_doc = DocumentTab::new(
                            &node_id,
                            &node_name,
                            &node_path,
                            &format!(
                                "// Contents of {}\n\npub fn {}() {{\n    // File loaded\n}}\n",
                                node_path,
                                node_name.replace('.', "_")
                            ),
                        );
                        docs.push(new_doc);
                        active_sig_item.set(docs.len() - 1);
                        docs_sig_item.set(docs);
                    }
                }
            })
            .into();
        btn
    });

    let header_label: Widget = Text::new(strings.project_tree)
        .style(
            TextStyle::new()
                .font_size(11.0)
                .font_weight(FontWeight::BOLD)
                .color(theme.text_muted),
        )
        .into();

    let total_nodes_count = state.project.nodes.len();
    let header_items: Vec<Widget> = vec![
        header_label,
        Spacer::new().into(),
        Text::new(format!("{total_nodes_count} nodes"))
            .style(TextStyle::new().font_size(10.0).color(theme.text_muted))
            .into(),
    ];

    let header = Container::new()
        .padding(EdgeInsets::all(8.0))
        .child(Row::new(header_items).alignment(CrossAxisAlignment::Center));

    let tree_col: Vec<Widget> = vec![header.into(), Expanded::new(tree_list).into()];

    Container::new()
        .color(theme.surface)
        .child(Column::new(tree_col))
        .into()
}
