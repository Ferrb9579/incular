//! Shared QA plumbing for the runnable examples.
//!
//! Every example owns its scenario declaration and test module.  This file
//! only provides the small amount of infrastructure needed to run those
//! scenarios against a live application without involving the host desktop.

use incular::prelude::{Code, Offset};
use incular::testing::{Simulation, SimulationError};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Enables the in-process example simulation when set to any value.
pub const SIMULATION_ENV: &str = "INCULAR_EXAMPLE_SIMULATION";
/// Makes a simulation process exit after its scenario completes.
pub const EXIT_AFTER_SIMULATION_ENV: &str = "INCULAR_EXAMPLE_SIMULATION_EXIT";
/// Overrides the directory used for captured application-frame screenshots.
pub const SCREENSHOT_DIR_ENV: &str = "INCULAR_EXAMPLE_SCREENSHOT_DIR";

/// The deterministic interaction contract owned by one example.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExampleScenario {
    pub name: &'static str,
    pub click_labels: &'static [&'static str],
    pub probe_points: &'static [(f32, f32)],
    pub scroll_delta: Option<(f32, f32)>,
    pub text_input: Option<&'static str>,
}

impl ExampleScenario {
    pub const fn new(
        name: &'static str,
        click_labels: &'static [&'static str],
        probe_points: &'static [(f32, f32)],
        scroll_delta: Option<(f32, f32)>,
        text_input: Option<&'static str>,
    ) -> Self {
        Self {
            name,
            click_labels,
            probe_points,
            scroll_delta,
            text_input,
        }
    }
}

/// Validates the structural part of an example's QA contract.
#[cfg(test)]
pub fn assert_scenario(scenario: ExampleScenario) {
    assert!(
        !scenario.name.trim().is_empty(),
        "scenario name is required"
    );
    assert!(
        scenario
            .click_labels
            .iter()
            .all(|label| !label.trim().is_empty()),
        "scenario labels must not be empty"
    );
    assert!(
        scenario
            .probe_points
            .iter()
            .all(|(x, y)| x.is_finite() && y.is_finite() && *x >= 0. && *y >= 0.),
        "scenario probe points must be finite and non-negative"
    );
    assert!(
        scenario
            .scroll_delta
            .is_none_or(|(x, y)| x.is_finite() && y.is_finite()),
        "scenario scroll deltas must be finite"
    );
    assert!(
        scenario.text_input.is_none_or(|text| !text.is_empty()),
        "scenario text input must not be empty"
    );
}

/// Starts a scenario only when the caller explicitly opts into simulation.
pub fn spawn_if_requested(
    simulation: Simulation,
    scenario: impl FnOnce(Simulation) + Send + 'static,
) {
    if std::env::var_os(SIMULATION_ENV).is_none() {
        return;
    }

    std::thread::Builder::new()
        .name("incular-example-simulation".into())
        .spawn(move || scenario(simulation))
        .expect("example simulation thread should start");
}

/// Runs the common pointer, keyboard, scrolling, and capture smoke sequence.
#[allow(dead_code)]
pub fn run_smoke(simulation: Simulation, scenario: ExampleScenario) {
    if let Err(error) = run_smoke_inner(&simulation, scenario) {
        eprintln!("example simulation [{}] failed: {error}", scenario.name);
        if std::env::var_os(EXIT_AFTER_SIMULATION_ENV).is_some() {
            std::process::exit(1);
        }
        return;
    }

    eprintln!("example simulation [{}] completed", scenario.name);
    if std::env::var_os(EXIT_AFTER_SIMULATION_ENV).is_some() {
        std::process::exit(0);
    }
}

/// Runs an example-owned interaction sequence with the same lifecycle and
/// capture handling as [`run_smoke`]. The callback is intentionally supplied
/// by the example so stateful controls can be checked instead of silently
/// treated as optional labels.
#[allow(dead_code)]
pub fn run_custom(
    simulation: Simulation,
    scenario: ExampleScenario,
    exercise: impl FnOnce(&Simulation) -> Result<(), SimulationError>,
) {
    let result = run_custom_inner(&simulation, scenario, exercise);
    match result {
        Ok(()) => {
            eprintln!("example simulation [{}] completed", scenario.name);
            if std::env::var_os(EXIT_AFTER_SIMULATION_ENV).is_some() {
                std::process::exit(0);
            }
        }
        Err(error) => {
            eprintln!("example simulation [{}] failed: {error}", scenario.name);
            if std::env::var_os(EXIT_AFTER_SIMULATION_ENV).is_some() {
                std::process::exit(1);
            }
        }
    }
}

#[allow(dead_code)]
fn run_custom_inner(
    simulation: &Simulation,
    scenario: ExampleScenario,
    exercise: impl FnOnce(&Simulation) -> Result<(), SimulationError>,
) -> Result<(), SimulationError> {
    wait_for_settled_frame(simulation)?;
    capture_to_disk(simulation, scenario.name, "initial")?;
    exercise(simulation)?;
    simulation.release_all_keys()?;
    capture_to_disk(simulation, scenario.name, "after-interaction")
}

#[allow(dead_code)]
fn run_smoke_inner(
    simulation: &Simulation,
    scenario: ExampleScenario,
) -> Result<(), SimulationError> {
    wait_for_settled_frame(simulation)?;
    let initial = simulation.capture()?;
    let scroll_target = Offset::new(initial.width() as f32 * 0.5, initial.height() as f32 * 0.5);
    write_capture(scenario.name, "initial", &initial)?;

    for label in scenario.click_labels {
        click_label_with_scroll(simulation, scenario, scroll_target, label)?;
    }

    for &(x, y) in scenario.probe_points {
        simulation.click_at(Offset::new(x, y))?;
    }

    if let Some(text) = scenario.text_input {
        simulation.type_text(text)?;
        simulation.press(Code::Backspace)?;
    }

    // Tab exercises the same normalized keyboard path as a native adapter.
    simulation.press(Code::Tab)?;

    if let Some((x, y)) = scenario.scroll_delta {
        simulation.move_mouse_to(scroll_target)?;
        simulation.scroll(Offset::new(x, y))?;
        wait_for_settled_frame(simulation)?;
    }

    simulation.release_all_keys()?;
    capture_to_disk(simulation, scenario.name, "after-interaction")?;
    Ok(())
}

#[allow(dead_code)]
fn click_label_with_scroll(
    simulation: &Simulation,
    scenario: ExampleScenario,
    scroll_target: Offset,
    label: &str,
) -> Result<(), SimulationError> {
    let search_scroll = scenario.scroll_delta.filter(|(x, y)| *x != 0. || *y != 0.);
    const MAX_SEARCH_SCROLLS: usize = 16;

    if let Some((x, y)) = search_scroll {
        // Try the current tree first. This matters for transient UI: a menu,
        // dialog, drawer, or snackbar may be the correct target for the next
        // command and resetting the scroll position would dismiss or obscure
        // it before the click is dispatched.
        match simulation.click(label) {
            Ok(()) => {
                wait_for_settled_frame(simulation)?;
                eprintln!(
                    "example simulation [{}] clicked {label:?} without search scrolling",
                    scenario.name
                );
                return Ok(());
            }
            Err(SimulationError::ElementNotFound(_)) => {}
            Err(error) => return Err(error),
        }

        // Each label is an independent probe. Return to the start before
        // searching so a missed lazy child cannot leave the next probe at the
        // end of the scroll extent.
        simulation.move_mouse_to(scroll_target)?;
        let reset_factor = MAX_SEARCH_SCROLLS as f32 + 1.0;
        simulation.scroll(Offset::new(-x * reset_factor, -y * reset_factor))?;
        wait_for_settled_frame(simulation)?;
    }

    for attempt in 0..=MAX_SEARCH_SCROLLS {
        match simulation.click(label) {
            Ok(()) => {
                wait_for_settled_frame(simulation)?;
                eprintln!(
                    "example simulation [{}] clicked {label:?} after {attempt} search scrolls",
                    scenario.name
                );
                return Ok(());
            }
            Err(SimulationError::ElementNotFound(_))
                if search_scroll.is_some() && attempt < MAX_SEARCH_SCROLLS =>
            {
                let (x, y) = search_scroll.expect("search scroll was checked above");
                simulation.scroll(Offset::new(x, y))?;
                wait_for_settled_frame(simulation)?;
            }
            Err(SimulationError::ElementNotFound(_)) => {
                eprintln!(
                    "example simulation [{}] skipped unavailable label {label:?}",
                    scenario.name
                );
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

pub fn wait_for_settled_frame(simulation: &Simulation) -> Result<(), SimulationError> {
    // A retained layout may need one frame to install lazy children and a
    // second frame to paint their measured extents. Keep captures deterministic
    // without sleeping the application thread or depending on wall-clock time.
    const SETTLE_FRAMES: usize = 3;
    for _ in 0..SETTLE_FRAMES {
        simulation.wait_for_frame()?;
    }
    Ok(())
}

pub fn capture_to_disk(
    simulation: &Simulation,
    example_name: &str,
    stage: &str,
) -> Result<(), SimulationError> {
    let screenshot = simulation.capture()?;
    write_capture(example_name, stage, &screenshot)?;
    eprintln!(
        "example simulation [{example_name}] captured {}x{} to {}",
        screenshot.width(),
        screenshot.height(),
        screenshot_path(example_name, stage).display()
    );
    Ok(())
}

fn write_capture(
    example_name: &str,
    stage: &str,
    screenshot: &incular::testing::Screenshot,
) -> Result<(), SimulationError> {
    let path = screenshot_path(example_name, stage);
    write_ppm(&path, screenshot).map_err(|error| {
        SimulationError::CaptureUnavailable(format!("could not write {}: {error}", path.display()))
    })
}

fn screenshot_path(example_name: &str, stage: &str) -> PathBuf {
    let root = std::env::var_os(SCREENSHOT_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/example-review/screenshots"));
    root.join(example_name).join(format!("{stage}.ppm"))
}

fn write_ppm(path: &Path, screenshot: &incular::testing::Screenshot) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    write!(
        file,
        "P6\n{} {}\n255\n",
        screenshot.width(),
        screenshot.height()
    )?;
    for pixel in screenshot.pixels().chunks_exact(4) {
        file.write_all(&pixel[..3])?;
    }
    file.flush()
}
