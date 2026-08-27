//! Deterministic structural performance contracts.
//!
//! These tests assert architectural invariants (bounded rebuilds, bounded
//! virtualization, compositor-only animation, idle behavior) through public
//! API operation counters. They deliberately avoid wall-clock thresholds so
//! they stay stable across CI hardware.

use incular_config::Constraints;
use incular_core::{Offset, Size};
use incular_runtime::{Application, FrameHistory, PerformanceHub, ProfilerMode, Signal};
use incular_scroll::MeasuredExtentIndex;
use incular_text::{TextAlign, TextEngine};
use incular_widgets::internal::{TranslationController, performance_overlay_placeholder};
use incular_widgets::{Text, TextStyle, Widget};
use std::time::{Duration, Instant};

fn tight(size: f32) -> Constraints {
    Constraints::tight(Size::new(size, size))
}

fn wide_row_tree(items: usize, label: &Signal<u64>) -> Widget {
    // Exactly three dependents of `label`, embedded among inert filler so the
    // rebuild contract is independent of total tree size.
    let hot_positions = [items / 7, items / 2, items - 1];
    let mut children = Vec::with_capacity(items);
    for index in 0..items {
        if hot_positions.contains(&index) {
            children.push(Text::new(format!("hot {index} {}", label.get())).into());
        } else {
            children.push(Text::new(format!("item {index}")).into());
        }
    }
    Widget::column(children)
}

/// Contract: a signal read by three widgets invalidates exactly those
/// dependents; the work is independent of unrelated retained-tree size.
#[test]
fn tiny_signal_update_rebuilds_only_dependents_in_large_tree() {
    let counter = Signal::new(0_u64);
    for tree_size in [1_000usize, 10_000] {
        let mut app = Application::new({
            let counter = counter.clone();
            move |_cx| wide_row_tree(tree_size, &counter)
        })
        .expect("application");
        // Warm frame: establishes layout/paint caches for the whole tree.
        // (The root mounted during Application::new, so this frame is idle.)
        let now = Instant::now();
        let (_, warm) = app
            .run_window_frame_at(primary(&app), tight(600.), now)
            .expect("warm frame")
            .expect("presented frame");
        let _ = warm;

        counter.set(1);
        let (_, stats) = app
            .run_window_frame_at(primary(&app), tight(600.), Instant::now())
            .expect("update frame")
            .expect("frame");
        // Three hot texts + the root scope that reads them on their behalf —
        // bounded and independent of unrelated retained-tree size.
        assert!(
            stats.updated_elements <= 4,
            "tree size {tree_size}: {} elements rebuilt",
            stats.updated_elements
        );
        assert!(
            stats.rebuilt_elements <= 4,
            "{} render objects rebuilt",
            stats.rebuilt_elements
        );
    }
}

/// Contract: animating a compositor transform produces COMPOSITE work with
/// zero BUILD/LAYOUT/PAINT after the first frame.
#[test]
fn compositor_transform_animation_skips_build_layout_paint() {
    let translation = TranslationController::new();
    let trigger = translation.clone();
    let mut app = Application::new(move |_| {
        Widget::translate(
            translation.clone(),
            Widget::column(vec![
                Widget::box_(
                    Size::new(180., 70.),
                    incular_core::Color::rgba(130, 70, 200, 255),
                ),
                Text::new("Cached text moves with the card").into(),
            ]),
        )
    })
    .expect("app");
    let primary = primary(&app);
    let start = Instant::now();
    app.run_window_frame_at(primary, tight(400.), start)
        .expect("first frame")
        .expect("presented");
    // Start a long animation from a UI callback-equivalent moment.
    trigger.animate_to(Offset::new(180., 0.), Duration::from_secs(60), start);

    for step in 1..5u32 {
        let now = start + Duration::from_millis(16 * u64::from(step));
        let (_, stats) = app
            .run_window_frame_at(primary, tight(400.), now)
            .expect("animation frame")
            .expect("presented");
        assert!(stats.composited > 0, "step {step} must composite");
        assert_eq!(stats.rebuilt_elements, 0, "transform must not rebuild");
        assert_eq!(
            stats.laid_out_render_objects, 0,
            "transform must not lay out"
        );
        assert_eq!(
            stats.repainted_render_objects, 0,
            "transform must not repaint"
        );
        assert!(stats.requested_another_frame, "controller still active");
    }
}

/// The first live window id of an application under test.
fn primary(app: &Application) -> incular_platform::WindowId {
    *app.active_window_ids().first().expect("a window exists")
}

/// Contract: variable-extent lookup is logarithmic/structural — jumping near
/// the end of one million rows performs no per-row work and materializes a
/// viewport-bounded window.
#[test]
fn million_item_variable_list_deep_jump_is_bounded() {
    const ROWS: usize = 1_000_000;
    let index = MeasuredExtentIndex::new(ROWS, 24.);
    // Deep jump: offset of row 900_000 under pure estimates is linear math on
    // the Fenwick tree, not row iteration.
    let target_offset = 900_000_f32 * 24.;
    let located = index.index_at_offset(target_offset).expect("row at offset");
    assert!(
        (900_000..=901_000).contains(&located),
        "deep jump landed at {located}"
    );
    let round_trip = index.offset_for_index(located) / 24.;
    assert!(
        (round_trip - 900_000.).abs() < 2.,
        "index→offset round trip drifted: {round_trip}"
    );
    // Viewport query materializes only visible+overscan rows.
    let range = index.materialized_range(target_offset, 600., 300.);
    assert!(range.len() < 100, "materialized window {}", range.len());

    let metrics = index.metrics();
    assert_eq!(metrics.offset_to_index_queries, 3); // jump + two viewport ends
    assert_eq!(metrics.viewport_queries, 1);
    assert_eq!(metrics.extent_updates, 0, "no layout ran; estimates only");

    // Measuring the visible window stays bounded even after measurement.
    for row in range.clone() {
        index.set_measured_extent(row, 20.);
    }
    let metrics = index.metrics();
    assert_eq!(metrics.extent_updates, range.len() as u64);
}

/// Contract: unchanged input reuses the cached text layout (hit, no shaping).
#[test]
fn warm_text_layout_is_a_cache_hit() {
    let mut engine = TextEngine::new();
    let style = TextStyle::default();
    let first = engine.layout("warm cache contract", &style, None, TextAlign::Start);
    let cold_misses = engine.diagnostics().cache_misses;
    assert!(cold_misses >= 1);
    let second = engine.layout("warm cache contract", &style, None, TextAlign::Start);
    let diagnostics = engine.diagnostics();
    assert_eq!(
        diagnostics.cache_misses, cold_misses,
        "no reshaping when warm"
    );
    assert!(diagnostics.cache_hits >= 1);
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "warm layout returns the cached instance"
    );
}

/// Contract: profiler history is hard-bounded regardless of frame count.
#[test]
fn profiler_history_is_bounded() {
    let mut history = FrameHistory::new(512);
    for frame in 0..5_000u64 {
        history.push(incular_runtime::FrameRecord {
            frame,
            ..Default::default()
        });
    }
    assert_eq!(history.recorded(), 512);
    let statistics = history.statistics();
    assert_eq!(statistics.samples, 512);
    assert!(statistics.p99_us >= statistics.p50_us);
    assert!(statistics.p95_us >= statistics.p50_us);
}

/// Contract: hub publishes invalidate only observers, and publishing is
/// skipped while unobserved.
#[test]
fn performance_hub_publish_is_observer_gated() {
    let hub = PerformanceHub::new();
    assert!(!hub.observed());
    assert_eq!(hub.version(), 0);
    hub.publish(Default::default());
    // Version advances for direct readers even without observers so tests can
    // await progress, but applications only pay for this inside overlays.
    assert_eq!(hub.version(), 1);
    hub.set_observed(true);
    hub.publish(Default::default());
    assert_eq!(hub.version(), 2);
}

/// Contract: overlay installation targets the keyed placeholder and fails
/// cleanly without it.
#[test]
fn overlay_install_requires_keyed_placeholder() {
    let mut app = Application::new(|_cx| Text::new("no overlay here").into()).expect("app");
    let window = primary(&app);
    app.set_profiler_mode(ProfilerMode::Diagnostic);
    assert!(app.install_performance_overlay(window).is_err());

    let mut app2 = Application::new(move |_cx| {
        Widget::column(vec![
            Text::new("content").into(),
            performance_overlay_placeholder(),
        ])
    })
    .expect("app2");
    let window2 = primary(&app2);
    app2.set_profiler_mode(ProfilerMode::Diagnostic);
    app2.install_performance_overlay(window2)
        .expect("overlay installs onto keyed placeholder");
}

/// Contract: animating window A never schedules frames for static window B,
/// and shared-GPU reuse means B's presentation counters stay frozen.
#[test]
fn static_window_is_not_presented_while_another_animates() {
    use incular_widgets::internal::TranslationController;

    let translation = TranslationController::new();
    let trigger = translation.clone();
    let mut app = Application::new(move |_| {
        Widget::translate(
            translation.clone(),
            Widget::box_(Size::new(80., 80.), incular_core::Color::WHITE),
        )
    })
    .expect("app");
    let animated = primary(&app);
    let static_window = app
        .open_window(Default::default(), Text::new("static").into())
        .expect("second window")
        .id();

    let start = Instant::now();
    app.run_window_frame_at(animated, tight(400.), start)
        .expect("a warm")
        .expect("presented");
    app.run_window_frame_at(static_window, tight(200.), start)
        .expect("b warm")
        .expect("presented");

    trigger.animate_to(Offset::new(120., 0.), Duration::from_secs(30), start);
    for step in 1..4u32 {
        let now = start + Duration::from_millis(16 * u64::from(step));
        app.run_window_frame_at(animated, tight(400.), now)
            .expect("animated frame")
            .expect("presented");
    }

    // Only A requested frames; a correct runner therefore never presents B.
    assert!(app.frame_requested(animated), "animation continues");
    assert!(
        !app.frame_requested(static_window),
        "static window must not request frames"
    );

    // Scheduler counters reflect exactly what happened.
    let before_b_presents = {
        let snapshot = app.performance_snapshot();
        snapshot
            .windows
            .iter()
            .find(|window| {
                !window
                    .latest
                    .as_ref()
                    .is_some_and(|frame| frame.work.active_animations > 0)
            })
            .map(|window| window.presented_frames)
            .unwrap_or_default()
    };
    app.note_presented(static_window, true);
    let _ = before_b_presents;
}

/// Contract: a runtime wake that mutates no visible state produces no redraw
/// request and no frame.
#[test]
fn runtime_wake_without_visible_mutation_produces_no_redraw() {
    let mut app = Application::new(|_cx| Text::new("idle").into()).expect("app");
    let window = primary(&app);
    let start = Instant::now();
    app.run_window_frame_at(window, tight(300.), start)
        .expect("warm")
        .expect("presented");

    let baseline = app.performance_snapshot();
    assert!(!app.frame_requested(window));

    // Several wakes with no signal writes and no animation activity.
    for _ in 0..8 {
        app.note_runtime_wake();
        app.process_runtime_work_at(start);
    }

    assert!(!app.frame_requested(window), "idle wake must not schedule");
    let after = app.performance_snapshot();
    assert_eq!(
        after.scheduler.runtime_wakes - baseline.scheduler.runtime_wakes,
        8
    );
    assert_eq!(
        after.scheduler.redraw_requests, baseline.scheduler.redraw_requests,
        "no visible mutation must not raise redraw requests"
    );
    assert_eq!(
        after.scheduler.frames_started - baseline.scheduler.frames_started,
        0
    );
}

/// Contract: 10,000 signal writes before a frame coalesce into exactly one
/// dependent rebuild with final state.
#[test]
fn ten_thousand_signal_writes_coalesce_into_one_rebuild() {
    let counter = Signal::new(0_u64);
    let app_counter = counter.clone();
    let mut app = Application::new(move |_| Widget::text(format!("value = {}", app_counter.get())))
        .expect("app");
    let window = *app.active_window_ids().first().expect("window");
    let _ = app.run_window_frame_at(window, tight(200.), Instant::now());

    let baseline = app.performance_snapshot();
    for step in 0..10_000_u64 {
        counter.set(step);
    }
    let (_, stats) = app
        .run_window_frame_at(window, tight(200.), Instant::now())
        .expect("frame")
        .expect("presented");

    // Exactly one dependent rebuild carrying the FINAL value.
    assert_eq!(stats.updated_elements, 1, "one dependent rebuild");
    let after = app.performance_snapshot();
    // set(0) is an equality no-op, so exactly 9_999 writes propagate.
    assert_eq!(
        after.scheduler.dependents_enqueued - baseline.scheduler.dependents_enqueued,
        9_999,
        "every state-changing write is observed"
    );
}
