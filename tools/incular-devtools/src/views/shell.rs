use super::{
    APP_BACKGROUND, BORDER, SURFACE, TEXT_MUTED, TEXT_PRIMARY, compact_button, gap, ui_text,
};
use incular::prelude::*;
use incular::widgets::internal::ScrollView;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolView {
    #[default]
    Widgets,
    Console,
    Network,
    Performance,
    Memory,
    Application,
}

pub(crate) fn initial_tool_view() -> ToolView {
    match std::env::var("INCULAR_DEVTOOLS_VIEW")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "console" => ToolView::Console,
        "network" => ToolView::Network,
        "performance" => ToolView::Performance,
        "memory" => ToolView::Memory,
        "application" => ToolView::Application,
        "inspector" | "signals" | "widgets" => ToolView::Widgets,
        _ => ToolView::Widgets,
    }
}

pub(crate) struct ShellData {
    pub(crate) active_view: ToolView,
    pub(crate) tool_view: Signal<ToolView>,
    pub(crate) header: String,
    pub(crate) connected: bool,
    pub(crate) row_count: usize,
    pub(crate) search_field: Widget,
    pub(crate) tree_list: Widget,
    pub(crate) inspector_scroll: ScrollController,
    pub(crate) page_content: Widget,
}

pub(crate) fn build_shell(data: ShellData) -> Widget {
    let ShellData {
        active_view,
        tool_view,
        header,
        connected,
        row_count,
        search_field,
        tree_list,
        inspector_scroll,
        page_content,
    } = data;
    let mut tabs = Vec::new();
    for (view, label) in [
        (ToolView::Widgets, "Widgets"),
        (ToolView::Console, "Console"),
        (ToolView::Network, "Network"),
        (ToolView::Performance, "Performance"),
        (ToolView::Memory, "Memory"),
        (ToolView::Application, "Application"),
    ] {
        let selected = active_view == view;
        let tool_view = tool_view.clone();
        tabs.push(compact_button(label, selected, move || {
            tool_view.set(view);
        }));
    }
    let header_widget = Row::new([
        ui_text("Incular DevTools", 20., TEXT_PRIMARY),
        gap(14., 1.),
        ui_text(
            header,
            12.,
            if connected {
                super::SUCCESS
            } else {
                TEXT_MUTED
            },
        ),
    ]);
    LayoutBuilder::new(move |_, constraints| {
        let width = constraints.max_width.max(960.);
        let height = constraints.max_height.max(640.);
        let body_height = (height - 116.).max(1.);
        let tree_width = (width * 0.34).clamp(340., 470.);
        let tree_panel: Widget = DecoratedBox::new(Padding::all(
            14.,
            Column::new([
                ui_text("Widget tree", 16., TEXT_PRIMARY),
                ui_text(
                    format!("{row_count} visible retained widgets"),
                    12.,
                    TEXT_MUTED,
                ),
                gap(1., 10.),
                search_field.clone(),
                gap(1., 10.),
                Expanded::new(tree_list.clone()).into(),
            ]),
        ))
        .background(SURFACE)
        .border(Border::new(1., BORDER))
        .radius(10.)
        .into();
        let details_panel: Widget = DecoratedBox::new(ScrollView::vertical(
            inspector_scroll.clone(),
            Padding::all(20., page_content.clone()),
        ))
        .background(SURFACE)
        .border(Border::new(1., BORDER))
        .radius(10.)
        .into();
        let body: Widget = if active_view == ToolView::Widgets {
            Row::new([
                ConstrainedBox::new(
                    Constraints::tight(Size::new(tree_width, body_height)),
                    tree_panel,
                )
                .into(),
                gap(12., 1.),
                Expanded::new(details_panel).into(),
            ])
            .into()
        } else {
            ConstrainedBox::new(
                Constraints::tight(Size::new(width - 32., body_height)),
                details_panel,
            )
            .into()
        };
        SizedBox::from_size(Size::new(width, height))
            .child(
                DecoratedBox::new(Padding::all(
                    16.,
                    Column::new([
                        Row::new(tabs.clone()).spacing(6.).into(),
                        gap(1., 8.),
                        header_widget.clone().into(),
                        gap(1., 12.),
                        ConstrainedBox::new(
                            Constraints::tight(Size::new(width - 32., body_height)),
                            body,
                        )
                        .into(),
                    ])
                    .cross_axis_alignment(CrossAxisAlignment::Start),
                ))
                .background(APP_BACKGROUND),
            )
            .into()
    })
    .into()
}
