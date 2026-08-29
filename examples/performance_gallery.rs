//! Interactive performance gallery: selectable stress scenarios plus the
//! retained-frame profiler overlay.
//!
//! Run in release for representative numbers:
//!
//! ```text
//! cargo run --release -p incular --example performance_gallery
//! ```
//!
//! Scenarios are mounted one at a time through the retained tree, so switching
//! unmounts the previous scenario's elements. The overlay at the top-left is
//! repaint-contained and rebuilds only when the performance hub publishes.
use incular::material::RawMaterialButton;
use incular::prelude::*;
use incular::widgets::internal::{
    Effects, Key, PathView, ScrollView, TranslationController, performance_overlay_placeholder,
};
use std::time::{Duration, Instant};

const SCENARIOS: [&str; 15] = [
    "100k widgets",
    "1M fixed list",
    "1M variable list",
    "large text",
    "many images",
    "many paths",
    "gradients",
    "effects",
    "nested scroll",
    "gesture stress",
    "transform anim",
    "multi-window",
    "recon 10k",
    "keyed reorder",
    "doc edit",
];

const CANVAS: Color = Color::rgba(11, 16, 27, 255);
const SURFACE: Color = Color::rgba(22, 30, 45, 255);
const SURFACE_RAISED: Color = Color::rgba(29, 40, 59, 255);
const BORDER: Color = Color::rgba(58, 75, 103, 255);
const PRIMARY: Color = Color::rgba(82, 124, 255, 255);
const PRIMARY_SOFT: Color = Color::rgba(50, 74, 132, 255);
const TEXT_PRIMARY: Color = Color::rgba(242, 246, 255, 255);
const TEXT_MUTED: Color = Color::rgba(157, 174, 201, 255);
const SUCCESS: Color = Color::rgba(70, 205, 151, 255);

fn gallery_text(value: impl Into<String>, size: f32, color: Color) -> Widget {
    Text::new(value)
        .style(TextStyle {
            size,
            color,
            ..TextStyle::default()
        })
        .into()
}

fn panel(child: impl Into<Widget>) -> Widget {
    DecoratedBox::new(Padding::all(20., child))
        .background(SURFACE)
        .border(Border::new(1., BORDER))
        .radius(18.)
        .into()
}

fn gap(width: f32, height: f32) -> Widget {
    Widget::box_(Size::new(width, height), Color::TRANSPARENT)
}

fn viewport(size: Size, child: impl Into<Widget>) -> Widget {
    ConstrainedBox::new(Constraints::tight(size), child).into()
}

fn stress_row(label: impl Into<String>, height: f32, color: Color) -> Widget {
    Stack::aligned(
        Alignment::CENTER_LEFT,
        [
            Widget::fixed_box(Size::new(640., height), color),
            Padding::new(
                EdgeInsets::symmetric(14., 0.),
                gallery_text(label, 15., TEXT_PRIMARY),
            )
            .into(),
        ],
    )
    .into()
}

fn scenario_tile(
    index: usize,
    label: &'static str,
    selected: usize,
    scenario: Signal<usize>,
) -> Widget {
    let active = index == selected;
    let background = if active { PRIMARY } else { SURFACE_RAISED };
    let border = if active { PRIMARY } else { BORDER };
    GestureDetector::new(
        Stack::new([
            DecoratedBox::new(Widget::box_(Size::new(148., 58.), Color::TRANSPARENT))
                .background(background)
                .border(Border::new(1., border))
                .radius(12.)
                .into(),
            gallery_text(label, 15., TEXT_PRIMARY),
        ])
        .alignment(Alignment::CENTER),
    )
    .on_tap(move || {
        scenario.set(index);
    })
    .into()
}

fn navigation(selected: usize, scenario: &Signal<usize>) -> Widget {
    let tiles = SCENARIOS
        .iter()
        .enumerate()
        .map(|(index, &label)| scenario_tile(index, label, selected, scenario.clone()))
        .collect::<Vec<_>>();

    panel(Column::new([
        gallery_text("STRESS SCENARIOS", 13., TEXT_MUTED),
        gap(1., 8.),
        gallery_text("Choose a workload", 23., TEXT_PRIMARY),
        gap(1., 6.),
        gallery_text(
            "Switch instantly between scale, rendering, interaction, and reconciliation tests.",
            14.,
            TEXT_MUTED,
        ),
        gap(1., 18.),
        Wrap::new(tiles).spacing(10.).run_spacing(10.).into(),
    ]))
}

fn diagnostics_panel() -> Widget {
    panel(Column::new([
        Row::new([
            DecoratedBox::new(Widget::box_(Size::new(9., 9.), SUCCESS))
                .radius(5.)
                .into(),
            gap(10., 1.),
            gallery_text("PROFILER LIVE", 13., SUCCESS),
        ])
        .into(),
        gap(1., 10.),
        gallery_text("Frame diagnostics", 23., TEXT_PRIMARY),
        gap(1., 6.),
        gallery_text(
            "Build, layout, paint, composite, and retained-tree activity for the selected workload.",
            14.,
            TEXT_MUTED,
        ),
        gap(1., 18.),
        gallery_text("LIVE FRAME PHASES", 12., TEXT_MUTED),
        gap(1., 10.),
        gallery_text("BUILD  ·  retained updates", 15., TEXT_PRIMARY),
        gap(1., 8.),
        gallery_text("LAYOUT  ·  constraint solving", 15., TEXT_PRIMARY),
        gap(1., 8.),
        gallery_text("PAINT  ·  display lists", 15., TEXT_PRIMARY),
        gap(1., 8.),
        gallery_text("COMPOSITE  ·  GPU submission", 15., TEXT_PRIMARY),
        gap(1., 20.),
        gallery_text(
            "The retained profiler readout is isolated below, so observing a workload does not add measured rebuilds.",
            13.,
            TEXT_MUTED,
        ),
    ]))
}

fn gallery_header(selected: usize) -> Widget {
    let active = SCENARIOS[selected];
    DecoratedBox::new(Padding::new(
        EdgeInsets::symmetric(24., 18.),
        Row::new([
            Widget::from(Column::new([
                gallery_text("INCULAR LABS", 13., Color::rgba(126, 157, 255, 255)),
                gallery_text("Performance gallery", 30., TEXT_PRIMARY),
            ])),
            Widget::from(Spacer::new()),
            Widget::from(
                DecoratedBox::new(Padding::new(
                    EdgeInsets::symmetric(16., 10.),
                    Column::new([
                        gallery_text("ACTIVE WORKLOAD", 11., TEXT_MUTED),
                        gallery_text(active, 16., TEXT_PRIMARY),
                    ]),
                ))
                .background(PRIMARY_SOFT)
                .border(Border::new(1., Color::rgba(80, 111, 181, 255)))
                .radius(12.),
            ),
        ]),
    ))
    .background(SURFACE)
    .border(Border::new(1., BORDER))
    .radius(18.)
    .into()
}

fn main() {
    let scenario = Signal::new(0_usize);
    let translation = TranslationController::new();
    let animation_trigger = translation.clone();

    let shared_window_count = Signal::new(1_u32);
    let window_opener_signal = Signal::new(0_u32);
    let recon_tick = Signal::new(0_u64);
    let reorder_flip = Signal::new(0_u64);
    let doc_edits = Signal::new(0_u64);
    let gesture_hits = Signal::new(0_u32);
    let dashboard_scroll = ScrollController::new();

    // Auto-pilot: INCULAR_GALLERY_AUTOPILOT=1 cycles every scenario so a
    // single launch exercises the full gallery (useful for demos/CI runs).
    let autopilot = std::env::var("INCULAR_GALLERY_AUTOPILOT").is_ok();
    let mut app = Application::new_with_options(
        WindowOptions {
            title: "Incular performance gallery".into(),
            initial_logical_size: Size::new(1440., 900.),
            minimum_logical_size: Some(Size::new(900., 650.)),
            ..WindowOptions::default()
        },
        move |cx| {
            let selected = scenario.get();
            if autopilot {
                let advance = scenario.clone();
                cx.spawn_into(
                    async move {
                        tokio::time::sleep(Duration::from_secs(4)).await;
                        0_u32
                    },
                    move |_result, _runtime| {
                        advance.set((selected + 1) % SCENARIOS.len());
                    },
                );
            }

            let workload = scenario_view(
                selected,
                &scenario,
                &translation,
                &shared_window_count,
                &recon_tick,
                &reorder_flip,
                &doc_edits,
                &gesture_hits,
                cx,
            )
            .into();
            let navigation = navigation(selected, &scenario);
            let header = gallery_header(selected);
            let scroll = dashboard_scroll.clone();

            let dashboard: Widget = LayoutBuilder::new(move |constraints| {
                let width = constraints.max_width.max(640.);
                let height = constraints.max_height.max(640.);
                let content_width = (width - 48.).max(592.);
                let body_height = (height - 142.).max(640.);

                let body: Widget = if content_width >= 1_280. {
                    ConstrainedBox::new(
                        Constraints::tight(Size::new(content_width, body_height)),
                        Row::new([
                            ConstrainedBox::new(
                                Constraints::tight(Size::new(350., body_height)),
                                navigation.clone(),
                            )
                            .into(),
                            gap(16., 1.),
                            Expanded::new(panel(Center::new(workload.clone())))
                                .flex(3)
                                .into(),
                            gap(16., 1.),
                            ConstrainedBox::new(
                                Constraints::tight(Size::new(380., body_height)),
                                diagnostics_panel(),
                            )
                            .into(),
                        ]),
                    )
                    .into()
                } else {
                    ConstrainedBox::new(
                        Constraints::new(content_width, content_width, 0., f32::INFINITY),
                        Column::new([
                            navigation.clone(),
                            gap(1., 16.),
                            panel(Center::new(workload.clone())),
                            gap(1., 16.),
                            diagnostics_panel(),
                        ]),
                    )
                    .into()
                };

                SizedBox::from_size(Size::new(width, height))
                    .child(
                        DecoratedBox::new(ScrollView::vertical(
                            scroll.clone(),
                            Padding::all(
                                24.,
                                Column::new([
                                    ConstrainedBox::new(
                                        Constraints::new(
                                            content_width,
                                            content_width,
                                            0.,
                                            f32::INFINITY,
                                        ),
                                        header.clone(),
                                    )
                                    .into(),
                                    gap(1., 16.),
                                    body,
                                ]),
                            ),
                        ))
                        .background(CANVAS),
                    )
                    .into()
            })
            .into();

            Stack::new([
                dashboard,
                Positioned::new(performance_overlay_placeholder())
                    .top(500.)
                    .right(44.)
                    .into(),
            ])
            .into()
        },
    )
    .expect("valid application");
    app.set_profiler_mode(ProfilerMode::Diagnostic);
    app.set_refresh_rate_hz(Some(60.));

    let primary = *app.active_window_ids().first().expect("primary window");
    if let Err(error) = incular::install_performance_overlay(&mut app, primary) {
        eprintln!("performance overlay unavailable: {error:?}");
    }

    let _ = animation_trigger;
    let _ = window_opener_signal;
    incular::run(app).expect("native gallery");
}

#[allow(clippy::too_many_arguments)]
fn scenario_view(
    selected: usize,
    _scenario: &Signal<usize>,
    translation: &TranslationController,
    shared_window_count: &Signal<u32>,
    recon_tick: &Signal<u64>,
    reorder_flip: &Signal<u64>,
    doc_edits: &Signal<u64>,
    gesture_hits: &Signal<u32>,
    cx: &BuildContext,
) -> impl Into<Widget> {
    match selected {
        0 => hundred_thousand_widgets(),
        1 => million_fixed_list(),
        2 => million_variable_list(),
        3 => large_text_document(),
        4 => many_images(),
        5 => many_paths(),
        6 => gradients(),
        7 => effects_stack(),
        8 => nested_scroll(),
        9 => gesture_stress(gesture_hits),
        10 => transform_animation(translation),
        11 => match cx.window_opener() {
            Some(opener) => multi_window(shared_window_count, opener),
            None => Widget::text("window opening unavailable"),
        },
        12 => reconciliation_10k(recon_tick),
        13 => keyed_reorder(reorder_flip),
        14 => document_edit(doc_edits),
        _ => Widget::text("unknown"),
    }
}

fn labeled(title: &str, child: impl Into<Widget>) -> Widget {
    DecoratedBox::new(Padding::all(
        18.,
        Column::new([
            gallery_text(title, 22., TEXT_PRIMARY),
            gap(1., 14.),
            viewport(Size::new(680., 520.), child),
        ]),
    ))
    .background(SURFACE_RAISED)
    .border(Border::new(1., BORDER))
    .radius(16.)
    .into()
}

/// Parent rebuilds every flip; 10k children are byte-identical and must hit
/// the identical-widget bailout (watch Scan/Bail and Built on the overlay).
fn reconciliation_10k(ticks: &Signal<u64>) -> Widget {
    let generation = ticks.get();
    let rows: Vec<Widget> = (0..10_000)
        .map(|index| Widget::from(Text::new(format!("row {index}"))))
        .collect();
    let bump = ticks.clone();
    let controller = ScrollController::new();
    labeled(
        "Parent rebuilds; 10k identical children cost ~zero",
        Widget::column(vec![
            RawMaterialButton::new(format!("rebuild parent (generation {generation})"))
                .on_press(move || bump.update(|value| *value += 1))
                .into(),
            viewport(
                Size::new(680., 460.),
                ScrollView::vertical(controller, Widget::column(rows)),
            ),
        ]),
    )
}

/// Reverses 5k keyed children each generation: pure reorder must mount or
/// unmount nothing (overlay Built stays tiny; state travels with keys).
fn keyed_reorder(flip: &Signal<u64>) -> Widget {
    let generation = flip.get();
    let forward: Vec<Widget> = (0..5_000)
        .map(|index| {
            Widget::from(Text::new(format!("state {index}"))).with_key(Key::Value(index as u64))
        })
        .collect();
    let mut children = forward;
    if generation % 2 == 1 {
        children.reverse();
    }
    let toggle = flip.clone();
    let controller = ScrollController::new();
    labeled(
        "Keyed reorder of 5,000 children",
        Widget::column(vec![
            RawMaterialButton::new(format!("reverse (generation {generation})"))
                .on_press(move || toggle.update(|value| *value += 1))
                .into(),
            viewport(
                Size::new(680., 460.),
                ScrollView::vertical(controller, Widget::column(children)),
            ),
        ]),
    )
}

/// Large document where exactly one paragraph changes per generation.
fn document_edit(edits: &Signal<u64>) -> Widget {
    let generation = edits.get();
    let mut lines: Vec<Widget> = (0..300)
        .map(|index| {
            Widget::from(
                Text::new(format!(
                    "Paragraph {index}: the retained paragraph cache keeps documents cheap."
                ))
                .color(TEXT_PRIMARY),
            )
        })
        .collect();
    lines[150] = Text::new(format!(
        "Paragraph 150 EDITED {generation} times: only this paragraph reshapes."
    ))
    .color(Color::rgba(126, 157, 255, 255))
    .into();
    let bump = edits.clone();
    let controller = ScrollController::new();
    labeled(
        "300-paragraph document, one-line edits",
        Widget::column(vec![
            RawMaterialButton::new(format!("edit paragraph 150 ({generation})"))
                .on_press(move || bump.update(|value| *value += 1))
                .into(),
            viewport(
                Size::new(680., 460.),
                ScrollView::vertical(controller, Widget::column(lines)),
            ),
        ]),
    )
}

fn hundred_thousand_widgets() -> Widget {
    // 100k logical rows backed by a native fixed-extent sliver: materialized
    // work stays bounded while the logical child count is huge.
    let controller = ScrollController::new();
    labeled(
        "100k logical widgets (lazy sliver)",
        CustomScrollView::new(vec![Box::new(SliverFixedExtentList::new(
            100_000,
            36.,
            move |index| {
                stress_row(
                    format!("Retained widget {index}"),
                    36.,
                    Color::rgba(40 + (index % 5) as u8 * 20, 70, 140, 255),
                )
            },
        )) as Box<dyn Sliver>])
        .controller(controller),
    )
}

fn million_fixed_list() -> Widget {
    let controller = ScrollController::new();
    labeled(
        "1,000,000 fixed rows",
        CustomScrollView::new(vec![
            Box::new(SliverFixedExtentList::new(1_000_000, 40., |index| {
                GestureDetector::new(stress_row(
                    format!("Item {index}"),
                    40.,
                    if index % 2 == 0 {
                        Color::rgba(42, 67, 112, 255)
                    } else {
                        Color::rgba(35, 56, 94, 255)
                    },
                ))
                .on_tap(move || {
                    if index % 100_000 == 0 {
                        eprintln!("clicked Item {index}");
                    }
                })
            })) as Box<dyn Sliver>,
        ])
        .controller(controller),
    )
}

fn million_variable_list() -> Widget {
    let controller = ScrollController::new();
    labeled(
        "1,000,000 variable rows (jump near 900k by dragging)",
        CustomScrollView::new(vec![Box::new(SliverList::builder(1_000_000, 40., |item| {
            let height = if item % 3 == 0 { 56. } else { 28. };
            stress_row(
                format!("Variable item {item}  ·  {height:.0}px"),
                height,
                Color::rgba(60, 90 + (item % 4) as u8 * 30, 120, 255),
            )
        })) as Box<dyn Sliver>])
        .controller(controller),
    )
}

fn large_text_document() -> Widget {
    let paragraphs: Vec<Widget> = (0..400)
        .map(|paragraph| {
            Widget::from(
                Text::new(format!(
                    "Paragraph {paragraph}: the retained text cache keeps warm relayouts cheap."
                ))
                .color(TEXT_PRIMARY),
            )
        })
        .collect();
    let controller = ScrollController::new();
    labeled(
        "Large wrapped document",
        ScrollView::vertical(controller, Widget::column(paragraphs)),
    )
}

fn many_images() -> Widget {
    let checker = ImageHandle::from_rgba8(
        2,
        2,
        [
            255, 90, 90, 255, 60, 60, 60, 255, 60, 60, 60, 255, 255, 90, 90, 255,
        ],
    )
    .expect("generated image");
    let grid: Vec<Widget> = (0..200)
        .map(|index| {
            DecoratedBox::new(Padding::all(
                4.,
                Image::new(checker.clone())
                    .width(if index % 7 == 0 { 64. } else { 48. })
                    .height(48.)
                    .sampling(if index % 2 == 0 {
                        ImageSampling::Linear
                    } else {
                        ImageSampling::Nearest
                    }),
            ))
            .background(Color::rgba(16, 23, 36, 255))
            .radius(6.)
            .into()
        })
        .collect();
    let rows = grid
        .chunks(10)
        .map(|chunk| Widget::row(chunk.to_vec()))
        .collect::<Vec<_>>();
    let controller = ScrollController::new();
    labeled(
        "200 image instances over few textures",
        ScrollView::vertical(controller, Widget::column(rows)),
    )
}

fn star_path() -> std::sync::Arc<Path> {
    use std::sync::Arc;
    let mut builder = Path::builder();
    let points = [
        (50., 0.),
        (61., 35.),
        (98., 35.),
        (68., 56.),
        (79., 92.),
        (50., 70.),
        (21., 92.),
        (32., 56.),
        (2., 35.),
        (39., 35.),
    ];
    for (index, (x, y)) in points.into_iter().enumerate() {
        if index == 0 {
            builder.move_to(Offset::new(x, y));
        } else {
            builder.line_to(Offset::new(x, y));
        }
    }
    builder.close();
    Arc::new(builder.build())
}

fn many_paths() -> Widget {
    let star = star_path();
    let grid: Vec<Widget> = (0..120)
        .map(|index| {
            Padding::all(
                4.,
                PathView::new(star.clone())
                    .fill(Color::rgba((index * 17 % 255) as u8, 160, 220, 255))
                    .stroke(Color::WHITE, Stroke::default()),
            )
            .into()
        })
        .collect();
    let rows = grid
        .chunks(6)
        .map(|chunk| Widget::row(chunk.to_vec()))
        .collect::<Vec<_>>();
    let controller = ScrollController::new();
    labeled(
        "120 tessellated vector stars",
        ScrollView::vertical(controller, Widget::column(rows)),
    )
}

fn gradients() -> Widget {
    let stops = |colors: &[(f32, Color)]| {
        GradientStops::new(
            colors
                .iter()
                .map(|&(offset, color)| GradientStop { offset, color })
                .collect(),
        )
    };
    let linear = LinearGradient {
        start: Offset::ZERO,
        end: Offset::new(300., 0.),
        stops: stops(&[
            (0., Color::rgba(255, 90, 90, 255)),
            (0.5, Color::rgba(90, 255, 140, 255)),
            (1., Color::rgba(90, 140, 255, 255)),
        ]),
    };
    let radial = RadialGradient {
        center: Offset::new(70., 70.),
        radius: 70.,
        stops: stops(&[(0., Color::WHITE), (1., Color::rgba(20, 20, 40, 255))]),
    };
    let sweep = SweepGradient {
        center: Offset::new(70., 70.),
        start_angle: 0.,
        stops: stops(&[
            (0., Color::rgba(255, 210, 60, 255)),
            (1., Color::rgba(190, 60, 255, 255)),
        ]),
    };
    let rows = Widget::row(vec![
        DecoratedBox::new(Widget::box_(Size::new(300., 34.), Color::TRANSPARENT))
            .background(linear)
            .radius(8.)
            .into(),
        DecoratedBox::new(Widget::box_(Size::new(140., 140.), Color::TRANSPARENT))
            .background(radial)
            .into(),
        DecoratedBox::new(Widget::box_(Size::new(140., 140.), Color::TRANSPARENT))
            .background(sweep)
            .into(),
    ]);
    labeled("Gradient LUT brushes", rows)
}

fn effects_stack() -> Widget {
    let card = DecoratedBox::new(Widget::box_(
        Size::new(240., 120.),
        Color::rgba(70, 110, 200, 255),
    ))
    .radius(12.);
    let rows = Widget::column(vec![
        Effects::new(card.clone()).blur(8.).into(),
        Effects::new(card.clone())
            .drop_shadow(Offset::new(10., 10.), 12., Color::rgba(0, 0, 0, 180))
            .into(),
        Widget::opacity(0.4, card.clone().into()),
        Widget::color_filtered(incular_rendering::ColorFilter::grayscale(1.), card.into()),
    ]);
    labeled("Offscreen effect passes", rows)
}

fn nested_scroll() -> Widget {
    let outer = ScrollController::new();
    let inner = ScrollController::new();
    let inner_list: Widget =
        CustomScrollView::new(vec![Box::new(SliverList::builder(2_000, 36., |item| {
            Widget::fixed_box(
                Size::new(360., if item % 3 == 0 { 56. } else { 32. }),
                Color::rgba(45, 85 + (item % 4) as u8 * 24, 145, 255),
            )
        })) as Box<dyn Sliver>])
        .controller(inner.clone())
        .into();
    labeled(
        "Nested scrolling with boundary transfer",
        ScrollView::vertical(
            outer,
            Widget::column(vec![
                Widget::fixed_box(Size::new(360., 120.), Color::rgba(35, 45, 70, 255)),
                viewport(Size::new(360., 320.), inner_list),
                Widget::fixed_box(Size::new(360., 700.), Color::rgba(30, 38, 55, 255)),
            ]),
        ),
    )
}

fn gesture_stress(hits: &Signal<u32>) -> Widget {
    let mut cells: Vec<Widget> = Vec::with_capacity(300);
    for index in 0..300 {
        let counter = hits.clone();
        cells.push(
            GestureDetector::new(Widget::fixed_box(
                Size::new(72., 72.),
                Color::rgba(120, 60 + ((index * 13) % 150) as u8, 90, 255),
            ))
            .on_tap(move || {
                counter.update(|count| *count += 1);
            })
            .into(),
        );
    }
    let display = hits.clone();
    let mut rows: Vec<Widget> = vec![Widget::text(format!("taps: {}", display.get()))];
    for chunk in cells.chunks(6) {
        rows.push(Widget::row(chunk.to_vec()));
    }
    let controller = ScrollController::new();
    labeled(
        "300 independent gesture regions",
        ScrollView::vertical(controller, Widget::column(rows)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gallery_sliver_list_receives_a_bounded_nonempty_viewport() {
        let mut tree = incular_widgets::WidgetTree::new();
        tree.mount(hundred_thousand_widgets()).unwrap();
        tree.layout(Constraints::tight(Size::new(760., 640.)));

        let diagnostics = tree.sliver_viewport_diagnostics().unwrap();
        assert_eq!(diagnostics.logical_item_count, 100_000);
        assert!(diagnostics.viewport_extent > 0.);
        assert!(diagnostics.viewport_extent <= 520.);
        assert!(diagnostics.materialized_item_count > 0);
        assert!(diagnostics.materialized_item_count < 100);

        tree.mount(million_variable_list()).unwrap();
        tree.layout(Constraints::tight(Size::new(760., 640.)));
        let variable = tree.sliver_viewport_diagnostics().unwrap();
        assert_eq!(variable.logical_item_count, 1_000_000);
        assert!(variable.viewport_extent > 0.);
        assert!(variable.materialized_item_count > 0);
        assert!(variable.materialized_item_count < 100);
    }
}

fn transform_animation(translation: &TranslationController) -> Widget {
    let trigger = translation.clone();
    trigger.animate_to(
        Offset::new(420., 0.),
        Duration::from_secs(600),
        Instant::now(),
    );
    labeled(
        "Compositor-only transform animation (BUILD/LAYOUT/PAINT stay zero)",
        Widget::translate(
            trigger,
            Widget::column(vec![
                Widget::fixed_box(Size::new(240., 90.), Color::rgba(130, 70, 200, 255)),
                Widget::text("Cached picture moves without repaint"),
            ]),
        ),
    )
}

fn multi_window(shared: &Signal<u32>, manager: WindowOpener) -> Widget {
    let count = shared.get();
    let increment = shared.clone();
    let open_manager = manager.clone();
    labeled(
        "Shared state across windows",
        Widget::column(vec![
            RawMaterialButton::new(format!("Shared counter: {count}"))
                .on_press(move || increment.update(|value| *value += 1))
                .into(),
            RawMaterialButton::new("Open static sibling window")
                .on_press(move || {
                    let manager = open_manager.clone();
                    let opened = manager.open_window_with(
                        WindowOptions {
                            title: "Gallery sibling".into(),
                            ..WindowOptions::default()
                        },
                        |_cx| {
                            Widget::text("Static sibling window: it must never redraw on its own")
                        },
                    );
                    if opened.is_err() {
                        eprintln!("gallery sibling window unavailable");
                    }
                })
                .into(),
            Widget::text(
                "The sibling shares this Signal but stays idle unless you interact there.",
            ),
        ]),
    )
}
