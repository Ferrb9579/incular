#![allow(dead_code)]

#[path = "../main.rs"]
mod example;
#[test]
fn scenario_contract_is_valid() {
    crate::example::example_support::assert_scenario(crate::example::simulations::SCENARIO);
}

#[test]
fn gallery_sliver_list_receives_a_bounded_nonempty_viewport() {
    use crate::example::{hundred_thousand_widgets, million_variable_list};
    use incular::prelude::*;

    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(hundred_thousand_widgets()).unwrap();
    tree.layout(Constraints::tight(Size::new(760., 640.)))
        .expect("layout");

    let diagnostics = tree.sliver_viewport_diagnostics().unwrap();
    assert_eq!(diagnostics.logical_item_count, 100_000);
    assert!(diagnostics.viewport_extent > 0.);
    assert!(diagnostics.viewport_extent <= 520.);
    assert!(diagnostics.materialized_item_count > 0);
    assert!(diagnostics.materialized_item_count < 100);

    tree.mount(million_variable_list()).unwrap();
    tree.layout(Constraints::tight(Size::new(760., 640.)))
        .expect("layout");
    let variable = tree.sliver_viewport_diagnostics().unwrap();
    assert_eq!(variable.logical_item_count, 1_000_000);
    assert!(variable.viewport_extent > 0.);
    assert!(variable.materialized_item_count > 0);
    assert!(variable.materialized_item_count < 100);
}

#[test]
fn multi_window_action_is_reachable_after_selecting_workload() {
    use incular::prelude::*;
    use std::{thread, time::Duration};

    let shared = Signal::new(0_u32);
    let shared_for_build = shared.clone();
    let mut application = Application::new(move |cx| {
        crate::example::multi_window(
            &shared_for_build,
            cx.window_opener()
                .expect("application owns a window opener"),
        )
    })
    .expect("performance workload should build");
    let simulation = application.simulation();
    let worker = thread::spawn(move || simulation.click("Open sibling window"));

    for _ in 0..2_000 {
        application.process_simulation_requests();
        if worker.is_finished() {
            let result = worker.join().expect("simulation worker should not panic");
            assert!(result.is_ok(), "selector simulation failed: {result:?}");
            let windows = application.active_window_ids();
            assert_eq!(windows.len(), 2);

            let sibling = application.simulation().for_window(windows[1]);
            let sibling_worker = thread::spawn(move || sibling.click("Shared counter: 0"));
            for _ in 0..2_000 {
                application.process_simulation_requests();
                if sibling_worker.is_finished() {
                    let result = sibling_worker
                        .join()
                        .expect("sibling simulation worker should not panic");
                    assert!(result.is_ok(), "sibling simulation failed: {result:?}");
                    assert_eq!(shared.get(), 1);
                    return;
                }
                thread::park_timeout(Duration::from_millis(1));
            }
            panic!("sibling simulation request was not serviced");
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }

    panic!("multi-window simulation request was not serviced");
}

#[test]
fn multi_window_action_is_reachable_after_selector_click() {
    use incular::prelude::*;
    use incular::widgets::internal::TranslationController;
    use std::time::Duration;

    use crate::example::{navigation, scenario_view};

    let selected = Signal::new(0_usize);
    let translation = TranslationController::new();
    let shared = Signal::new(0_u32);
    let recon_tick = Signal::new(0_u64);
    let reorder_flip = Signal::new(0_u64);
    let doc_edits = Signal::new(0_u64);
    let gesture_hits = Signal::new(0_u32);
    let selected_for_build = selected.clone();
    let translation_for_build = translation.clone();
    let shared_for_build = shared.clone();
    let recon_for_build = recon_tick.clone();
    let reorder_for_build = reorder_flip.clone();
    let edits_for_build = doc_edits.clone();
    let hits_for_build = gesture_hits.clone();
    let navigation_selection = selected_for_build.clone();
    let mut application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(1440., 900.),
            ..WindowOptions::default()
        },
        move |cx| {
            Widget::from(Column::new(Vec::<Widget>::from([
                navigation(selected_for_build.get(), &navigation_selection),
                scenario_view(
                    selected_for_build.get(),
                    &selected_for_build,
                    &translation_for_build,
                    &shared_for_build,
                    &recon_for_build,
                    &reorder_for_build,
                    &edits_for_build,
                    &hits_for_build,
                    cx,
                )
                .into(),
            ])))
        },
    )
    .expect("performance gallery selector should build");
    let simulation = application.simulation();
    let worker = std::thread::spawn(move || {
        simulation
            .click("multi-window")
            .and_then(|_| simulation.click("Open sibling window"))
    });

    for _ in 0..2_000 {
        application.process_simulation_requests();
        if worker.is_finished() {
            let result = worker.join().expect("simulation worker should not panic");
            assert!(result.is_ok(), "selector simulation failed: {result:?}");
            assert_eq!(application.active_window_ids().len(), 2);
            return;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }

    panic!("selector simulation request was not serviced");
}

#[test]
fn diagnostic_overlay_does_not_block_lower_navigation_tiles() {
    use incular::prelude::*;
    use incular::widgets::internal::performance_overlay_placeholder;
    use std::time::Duration;

    let selected = Signal::new(0_usize);
    let selection_for_build = selected.clone();
    let mut application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(1440., 900.),
            ..WindowOptions::default()
        },
        move |_| {
            Stack::new([
                Widget::from(ConstrainedBox::new(
                    Constraints::tight(Size::new(350., 700.)),
                    crate::example::navigation(0, &selection_for_build),
                )),
                Positioned::new(performance_overlay_placeholder())
                    .top(500.)
                    .right(44.)
                    .into(),
            ])
            .into()
        },
    )
    .expect("performance gallery overlay test should build");
    application.set_profiler_mode(ProfilerMode::Diagnostic);
    let window = application.primary_window();
    application
        .install_performance_overlay(window)
        .expect("performance overlay should install");

    let simulation = application.simulation();
    let worker = std::thread::spawn(move || simulation.click("doc edit"));
    for _ in 0..2_000 {
        application.process_simulation_requests();
        if worker.is_finished() {
            let result = worker.join().expect("simulation worker should not panic");
            assert!(result.is_ok(), "lower navigation click failed: {result:?}");
            assert_eq!(selected.get(), 14);
            return;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }

    panic!("lower navigation simulation request was not serviced");
}
