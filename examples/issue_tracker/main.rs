//! Native reproduction of the pinned QuickGUI/Electron issue-tracker-v1 workload.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use incular::controls::{
    Button, ButtonStyle, ControlTheme, ControlThemeScope, StateColor, TextArea, TextField,
    TextFieldStyle,
};
use incular::prelude::*;
use incular::text::TextEditingController;
use incular::widgets::internal::ScrollView;
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

#[path = "tests/automation.rs"]
mod automation;

#[derive(Clone, Deserialize)]
struct Issue {
    id: String,
    title: String,
    project: String,
    owner: String,
    priority: String,
    status: String,
    description: String,
    notes: String,
}

const INK: Color = Color::rgba(32, 36, 44, 255);
const MUTED: Color = Color::rgba(115, 124, 140, 255);
const LINE: Color = Color::rgba(226, 229, 235, 255);
const BLUE: Color = Color::rgba(40, 91, 212, 255);

fn text_style(size: f32, height: f32, weight: FontWeight, color: Color) -> TextStyle {
    TextStyle::new()
        .font_family("Arial")
        .font_size(size)
        .font_weight(weight)
        .line_height_absolute(height)
        .color(color)
}

fn text(
    value: impl Into<String>,
    size: f32,
    height: f32,
    weight: FontWeight,
    color: Color,
) -> Widget {
    Text::new(value)
        .style(text_style(size, height, weight, color))
        .into()
}

fn label(value: impl Into<String>, size: f32, height: f32) -> Widget {
    text(value, size, height, FontWeight::W400, INK)
}

fn gap(height: f32) -> Widget {
    SizedBox::new().height(height).into()
}

fn column(children: Vec<Widget>) -> Column {
    Column::new(children)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Start)
}

fn between(left: Widget, right: Widget) -> Widget {
    Row::new(vec![left, Spacer::new().into(), right])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .into()
}

fn button_style(width: f32, height: f32, background: Color) -> ButtonStyle {
    ButtonStyle {
        fixed_size: Some(Size::new(width, height)),
        background: Some(background),
        foreground: Some(INK),
        border: Some(Border::new(0.0, Color::TRANSPARENT)),
        border_radius: Some(7.0),
        padding: Some(EdgeInsets::symmetric(12.0, 0.0)),
        text_style: Some(text_style(14.0, 16.0, FontWeight::W400, INK)),
        overlay_color: Some(Color::TRANSPARENT.into()),
        animation_duration: Some(std::time::Duration::ZERO),
        ..ButtonStyle::default()
    }
}

fn input_style(background: Color, padding: EdgeInsets) -> TextFieldStyle {
    TextFieldStyle {
        background: Some(background),
        foreground: Some(INK),
        placeholder_color: Some(Color::rgba(117, 117, 117, 255)),
        border: Some(Border::new(1.0, Color::rgba(220, 224, 231, 255))),
        border_focused: Some(Border::new(2.0, BLUE)),
        border_radius: Some(7.0),
        padding: Some(padding),
    }
}

// Explicit one-pixel separators preserve the source CSS's individual border edges.
#[allow(clippy::too_many_arguments)]
fn ruled(
    child: Widget,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
    rw: f32,
    rh: f32,
    color: Color,
) -> Widget {
    Container::new()
        .width(width)
        .height(height)
        .child(Stack::new(vec![
            child,
            Positioned::new(IgnorePointer::new(
                Container::new().width(rw).height(rh).color(color),
            ))
            .left(x)
            .top(y)
            .into(),
        ]))
        .into()
}

// A native Windows-style scrollbar. Its thumb observes scroll metrics, so wheel,
// arrow, track, and drag interactions all share the actual viewport controller.
fn scroll_area(
    controller: &ScrollController,
    changed: &Signal<u64>,
    width: f32,
    height: f32,
    gutter: f32,
    child: Widget,
) -> Widget {
    let mut overlay = controller.scrollbar_style();
    overlay.width = 0.0;
    controller.set_scrollbar_style(overlay);
    let viewport: Widget = Container::new()
        .width(width)
        .height(height)
        .padding(EdgeInsets::only(0.0, 0.0, gutter, 0.0))
        .child(ScrollView::vertical(controller.clone(), child))
        .into();
    let controller = controller.clone();
    let changed = changed.clone();
    let bar = LayoutBuilder::new(move |_, _| {
        let _ = changed.get();
        let max = controller.max_offset();
        if max <= 0.0 {
            return SizedBox::shrink().into();
        }
        let track = height - 36.0;
        let thumb = (track * controller.viewport_extent() / controller.content_extent()).max(18.0);
        let travel = track - thumb;
        let top = 18.0 + travel * controller.offset() / max;
        let mut children: Vec<Widget> = vec![
            GestureDetector::new(
                Container::new()
                    .width(15.0)
                    .height(height)
                    .color(Color::WHITE),
            )
            .on_tap_down({
                let controller = controller.clone();
                move |details| {
                    controller.scroll_by(if details.local_position.y < top {
                        -height
                    } else {
                        height
                    });
                }
            })
            .into(),
        ];
        children.push(
            Positioned::new(
                GestureDetector::new(
                    Container::new()
                        .width(9.0)
                        .height(thumb)
                        .radius(4.5)
                        .color(Color::rgba(139, 139, 139, 255)),
                )
                .on_vertical_drag_update({
                    let controller = controller.clone();
                    move |delta| {
                        controller.scroll_by(delta.y * max / travel.max(1.0));
                    }
                }),
            )
            .left(3.0)
            .top(top)
            .into(),
        );
        for (top, symbol, delta) in [(0.0, "▲", -40.0), (height - 15.0, "▼", 40.0)] {
            let controller = controller.clone();
            children.push(
                Positioned::new(
                    Button::new(symbol)
                        .style(ButtonStyle {
                            foreground: Some(Color::rgba(139, 139, 139, 255)),
                            text_style: Some(text_style(12.0, 12.0, FontWeight::W400, MUTED)),
                            padding: Some(EdgeInsets::ZERO),
                            border_radius: Some(0.0),
                            ..button_style(15.0, 15.0, Color::WHITE)
                        })
                        .on_click(move || {
                            controller.scroll_by(delta);
                        }),
                )
                .top(top)
                .left(0.0)
                .into(),
            );
        }
        Container::new()
            .width(15.0)
            .height(height)
            .child(Stack::new(children))
            .into()
    });
    Container::new()
        .width(width)
        .height(height)
        .child(Stack::new(vec![
            viewport,
            Positioned::new(bar).right(0.0).top(0.0).into(),
        ]))
        .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let issues: Vec<Issue> = serde_json::from_str(include_str!("issues.json"))?;
    assert_eq!(issues.len(), 1000);
    let notes = Rc::new(RefCell::new((
        0_usize,
        TextEditingController::with_text(&issues[0].notes),
    )));
    let issues = Rc::new(RefCell::new(issues));
    let selected = Signal::new(0_usize);
    let filter = Signal::new("All issues".to_owned());
    let query = Signal::new(String::new());
    let page = Signal::new(0_usize);
    let revision = Signal::new(0_u64);
    let search = TextEditingController::new();
    let scroll = ScrollController::new();
    let details_scroll = ScrollController::new();
    let probe = automation::Probe::new();
    let scroll_changed = Signal::new(0_u64);
    let details_changed = Signal::new(0_u64);
    let _scroll_subscription = scroll.add_listener({
        let changed = scroll_changed.clone();
        let probe = probe.clone();
        move |event| {
            probe.observe_scroll(false, event.metrics.pixels);
            changed.update(|value| *value += 1);
            false
        }
    });
    let _details_subscription = details_scroll.add_listener({
        let changed = details_changed.clone();
        let probe = probe.clone();
        move |event| {
            probe.observe_scroll(true, event.metrics.pixels);
            changed.update(|value| *value += 1);
            false
        }
    });
    let app_probe = probe.clone();
    let app = Application::new_with_options(
        WindowOptions {
            title: "Issue tracker — ready".into(),
            initial_logical_size: Size::new(1100.0, 720.0),
            size_policy: incular::platform::WindowSizePolicy::Viewport,
            resizable: false,
            background_color: Color::WHITE,
            ..WindowOptions::default()
        },
        move |_| {
            let _ = revision.get();
            let active_filter = filter.get();
            let needle = query.get().trim().to_lowercase();
            let selected_index = selected.get();
            let data = issues.borrow();
            let completed = data.iter().filter(|issue| issue.status == "Done").count();
            let matching: Vec<usize> = data
                .iter()
                .enumerate()
                .filter(|(_, issue)| {
                    (active_filter == "All issues"
                        || (active_filter == "Open" && issue.status != "Done")
                        || (active_filter == "Completed" && issue.status == "Done"))
                        && format!(
                            "{} {} {} {}",
                            issue.id, issue.title, issue.project, issue.owner
                        )
                        .to_lowercase()
                        .contains(&needle)
                })
                .map(|(index, _)| index)
                .collect();
            let pages = matching.len().div_ceil(100).max(1);
            let current_page = page.get().min(pages - 1);
            let row_width = if matching.len().min(100) > 7 {
                559.0
            } else {
                574.0
            };
            let rows: Vec<Widget> = matching
                .iter()
                .skip(current_page * 100)
                .take(100)
                .map(|&index| {
                    let issue = &data[index];
                    let selected = selected.clone();
                    let background = if index == selected_index {
                        Color::rgba(237, 243, 255, 255)
                    } else {
                        Color::WHITE
                    };
                    let button = Button::with_child(column(vec![
                        Text::new(&issue.title)
                            .style(text_style(14.0, 16.0, FontWeight::W500, INK))
                            .max_lines(Some(1))
                            .soft_wrap(false)
                            .overflow(TextOverflow::Ellipsis)
                            .into(),
                        gap(8.0),
                        Text::new(format!(
                            "{} · {} · {} · {}",
                            issue.id, issue.project, issue.status, issue.owner
                        ))
                        .style(text_style(11.0, 12.0, FontWeight::W400, MUTED))
                        .max_lines(Some(1))
                        .soft_wrap(false)
                        .into(),
                    ]))
                    .style(ButtonStyle {
                        border_radius: Some(0.0),
                        padding: Some(EdgeInsets::only(20.0, 0.0, 20.0, 1.0)),
                        alignment: Some(Alignment::CENTER_LEFT),
                        background_states: Some(StateColor::new(background).hovered(
                            if index == selected_index {
                                background
                            } else {
                                Color::rgba(247, 249, 252, 255)
                            },
                        )),
                        ..button_style(row_width, 68.0, background)
                    })
                    .on_click(move || {
                        selected.set(index);
                    });
                    ruled(
                        button.into(),
                        row_width,
                        68.0,
                        0.0,
                        67.0,
                        row_width,
                        1.0,
                        Color::rgba(237, 240, 244, 255),
                    )
                })
                .collect();
            let issue = data[selected_index].clone();
            app_probe.observe(|| automation::Observation {
                records: data.len(),
                rows: rows.len(),
                page: current_page,
                matches: matching.len(),
                selected: selected_index,
                status: issue.status.clone(),
                notes: issue.notes.clone(),
                list_scroll: scroll.offset(),
                details_scroll: details_scroll.offset(),
            });
            drop(data);
            let notes = {
                let mut current = notes.borrow_mut();
                if current.0 != selected_index {
                    *current = (
                        selected_index,
                        TextEditingController::with_text(&issue.notes),
                    );
                }
                current.1.clone()
            };
            let mut nav = vec![
                Container::with_child(text("Orbit", 22.0, 25.333334, FontWeight::W700, INK))
                    .padding(EdgeInsets::symmetric(12.0, 0.0))
                    .into(),
                gap(14.0),
                Container::with_child(text(
                    "Product workspace",
                    12.0,
                    14.0,
                    FontWeight::W400,
                    MUTED,
                ))
                .padding(EdgeInsets::symmetric(12.0, 0.0))
                .into(),
                gap(32.0),
            ];
            for status in ["All issues", "Open", "Completed"] {
                let filter = filter.clone();
                let page = page.clone();
                let scroll = scroll.clone();
                let is_active = status == active_filter;
                let background = if is_active {
                    Color::rgba(228, 235, 251, 255)
                } else {
                    Color::TRANSPARENT
                };
                nav.push(
                    Button::new(status)
                        .style(ButtonStyle {
                            foreground: Some(if is_active { BLUE } else { INK }),
                            text_style: Some(text_style(
                                14.0,
                                16.0,
                                if is_active {
                                    FontWeight::W600
                                } else {
                                    FontWeight::W400
                                },
                                INK,
                            )),
                            alignment: Some(Alignment::CENTER_LEFT),
                            background_states: Some(
                                StateColor::new(background)
                                    .hovered(Color::rgba(232, 235, 241, 255)),
                            ),
                            ..button_style(151.0, 40.0, background)
                        })
                        .on_click(move || {
                            filter.set(status.to_owned());
                            page.set(0);
                            scroll.jump_to(0.0);
                        })
                        .into(),
                );
                nav.push(gap(8.0));
            }
            nav.push(Spacer::new().into());
            nav.push(
                Container::with_child(text(
                    "September cycle\n4 projects · 5 teammates",
                    12.0,
                    18.0,
                    FontWeight::W400,
                    MUTED,
                ))
                .padding(EdgeInsets::all(12.0))
                .into(),
            );
            let sidebar: Widget = Container::new()
                .width(176.0)
                .height(720.0)
                .color(Color::rgba(244, 245, 247, 255))
                .padding(EdgeInsets::only(12.0, 24.0, 13.0, 24.0))
                .child(column(nav).main_axis_size(MainAxisSize::Max))
                .into();
            let mut theme = ControlTheme::light();
            theme.typography.body = text_style(14.0, 16.0, FontWeight::W400, INK);
            let toolbar: Widget = Container::new()
                .width(924.0)
                .height(94.0)
                .alignment(Alignment::CENTER)
                .padding(EdgeInsets::only(24.0, 0.0, 24.0, 1.0))
                .child(between(
                    column(vec![
                        text("Issue inbox", 24.0, 28.0, FontWeight::W700, INK),
                        gap(6.0),
                        text(
                            format!("{} open · {} completed", 1000 - completed, completed),
                            12.0,
                            14.0,
                            FontWeight::W400,
                            MUTED,
                        ),
                    ])
                    .into(),
                    TextField::new(search.clone())
                        .placeholder("Search issues, projects, people")
                        .size(Size::new(270.0, 36.0))
                        .style(input_style(
                            Color::rgba(248, 249, 251, 255),
                            EdgeInsets::only(5.0, 1.0, 5.0, 1.0),
                        ))
                        .on_changed({
                            let query = query.clone();
                            let page = page.clone();
                            let scroll = scroll.clone();
                            move |value| {
                                query.set(value);
                                page.set(0);
                                scroll.jump_to(0.0);
                            }
                        })
                        .into(),
                ))
                .into();
            let header: Widget = Container::new()
                .width(574.0)
                .height(48.0)
                .alignment(Alignment::CENTER)
                .padding(EdgeInsets::symmetric(20.0, 0.0))
                .color(Color::rgba(250, 251, 252, 255))
                .child(between(
                    text(
                        format!("{} issues", matching.len()),
                        12.0,
                        14.0,
                        FontWeight::W400,
                        MUTED,
                    ),
                    text("Updated this week", 12.0, 14.0, FontWeight::W400, MUTED),
                ))
                .into();
            let controls: Vec<Widget> = [
                ("Previous", current_page > 0, 72.0),
                ("Next", current_page + 1 < pages, 50.0),
            ]
            .into_iter()
            .map(|(name, enabled, width)| {
                let page = page.clone();
                let scroll = scroll.clone();
                Button::new(name)
                    .enabled(enabled)
                    .style(ButtonStyle {
                        foreground: Some(if enabled {
                            INK
                        } else {
                            Color::rgba(166, 167, 171, 255)
                        }),
                        border: Some(Border::new(
                            1.0,
                            if enabled {
                                Color::rgba(220, 224, 231, 255)
                            } else {
                                Color::rgba(241, 243, 245, 255)
                            },
                        )),
                        border_radius: Some(6.0),
                        text_style: Some(text_style(12.0, 14.0, FontWeight::W400, INK)),
                        ..button_style(width, 32.0, Color::WHITE)
                    })
                    .on_click(move || {
                        page.set(if name == "Next" {
                            current_page + 1
                        } else {
                            current_page.saturating_sub(1)
                        });
                        scroll.jump_to(0.0);
                    })
                    .into()
            })
            .collect();
            let footer: Widget = Container::new()
                .width(574.0)
                .height(58.0)
                .alignment(Alignment::CENTER)
                .padding(EdgeInsets::only(20.0, 1.0, 20.0, 0.0))
                .child(between(
                    label(
                        format!("Page {} of {}", current_page + 1, pages),
                        12.0,
                        14.0,
                    ),
                    Row::new(controls).spacing(8.0).into(),
                ))
                .into();
            let header = ruled(header, 574.0, 48.0, 0.0, 47.0, 574.0, 1.0, LINE);
            let footer = ruled(footer, 574.0, 58.0, 0.0, 0.0, 574.0, 1.0, LINE);
            let mut inbox_children = vec![header];
            if rows.is_empty() {
                // Electron puts its empty message after the shrinking flex viewport.
                inbox_children.push(SizedBox::new().width(574.0).height(456.0).into());
                inbox_children.push(
                    Container::with_child(text(
                        "No matching issues",
                        14.0,
                        16.0,
                        FontWeight::W400,
                        MUTED,
                    ))
                    .width(574.0)
                    .height(64.0)
                    .padding(EdgeInsets::all(24.0))
                    .into(),
                );
            } else {
                inbox_children.push(scroll_area(
                    &scroll,
                    &scroll_changed,
                    574.0,
                    520.0,
                    574.0 - row_width,
                    column(rows).into(),
                ));
            }
            inbox_children.push(footer);
            let inbox: Widget = column(inbox_children).into();
            let mut notes_theme = theme.clone();
            notes_theme.typography.body = text_style(13.0, 19.0, FontWeight::W400, INK);
            let metadata = column(
                [
                    ("Status", &issue.status),
                    ("Assignee", &issue.owner),
                    ("Priority", &issue.priority),
                ]
                .into_iter()
                .map(|(name, value)| {
                    between(
                        text(name, 12.0, 14.0, FontWeight::W400, MUTED),
                        text(value, 12.0, 14.0, FontWeight::W500, INK),
                    )
                })
                .collect(),
            )
            .spacing(12.0);
            let details_content: Widget = Container::new()
                .width(334.0)
                .padding(EdgeInsets::all(24.0))
                .child(column(vec![
                    text(
                        format!("{} / {}", issue.id, issue.project),
                        12.0,
                        14.0,
                        FontWeight::W400,
                        MUTED,
                    ),
                    gap(14.0),
                    text(&issue.title, 21.0, 28.0, FontWeight::W700, INK),
                    gap(22.0),
                    metadata.into(),
                    gap(24.0),
                    label(&issue.description, 13.0, 20.0),
                    gap(22.0),
                    text("Working notes", 12.0, 14.0, FontWeight::W600, INK),
                    gap(8.0),
                    ControlThemeScope::new(
                        notes_theme,
                        TextArea::new(notes)
                            .size(Size::new(280.0, 94.0))
                            .style(input_style(Color::WHITE, EdgeInsets::all(3.0)))
                            .on_changed({
                                let issues = issues.clone();
                                move |value| issues.borrow_mut()[selected_index].notes = value
                            }),
                    )
                    .into(),
                    gap(20.0),
                    Button::new(if issue.status == "Done" {
                        "Reopen issue"
                    } else {
                        "Mark complete"
                    })
                    .style(ButtonStyle {
                        foreground: Some(Color::WHITE),
                        text_style: Some(text_style(14.0, 16.0, FontWeight::W500, Color::WHITE)),
                        ..button_style(286.0, 36.0, BLUE)
                    })
                    .on_click({
                        let issues = issues.clone();
                        let revision = revision.clone();
                        move || {
                            let mut data = issues.borrow_mut();
                            let status = &mut data[selected_index].status;
                            *status = if *status == "Done" { "Open" } else { "Done" }.to_owned();
                            drop(data);
                            revision.update(|value| *value += 1);
                        }
                    })
                    .into(),
                    gap(10.0),
                    text(
                        "Changes are kept for this session.",
                        11.0,
                        12.0,
                        FontWeight::W400,
                        MUTED,
                    ),
                ]))
                .into();
            let details: Widget = Container::new()
                .width(350.0)
                .height(626.0)
                .padding(EdgeInsets::only(1.0, 0.0, 0.0, 0.0))
                .child(scroll_area(
                    &details_scroll,
                    &details_changed,
                    349.0,
                    626.0,
                    15.0,
                    details_content,
                ))
                .into();
            let sidebar = ruled(sidebar, 176.0, 720.0, 175.0, 0.0, 1.0, 720.0, LINE);
            let toolbar = ruled(toolbar, 924.0, 94.0, 0.0, 93.0, 924.0, 1.0, LINE);
            let details = ruled(details, 350.0, 626.0, 0.0, 0.0, 1.0, 626.0, LINE);
            ControlThemeScope::new(
                theme,
                Row::new(vec![
                    sidebar,
                    column(vec![
                        toolbar,
                        Row::new(vec![inbox, details])
                            .cross_axis_alignment(CrossAxisAlignment::Start)
                            .into(),
                    ])
                    .into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Start),
            )
            .into()
        },
    )?;
    automation::start(app.simulation(), probe);
    incular::run(app)?;
    Ok(())
}
