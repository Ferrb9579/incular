use crate::example_support::{self, ExampleScenario};
use incular::prelude::{Code, Offset};
use incular::testing::{Simulation, SimulationError};

const NAVIGATION_LABELS: [&str; 15] = [
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

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "performance_gallery",
    &NAVIGATION_LABELS,
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 300.)),
    None,
);

pub fn run(simulation: Simulation) {
    example_support::run_custom(simulation, SCENARIO, exercise_gallery);
}

fn exercise_gallery(simulation: &Simulation) -> Result<(), SimulationError> {
    for (index, label) in NAVIGATION_LABELS.iter().enumerate() {
        click(simulation, label)?;

        match index {
            9 => {
                click(simulation, "gesture cell 0 (tap count 0)")?;
                click(simulation, "gesture cell 0 (tap count 1)")?;
            }
            11 => {
                click(simulation, "Shared counter: 1")?;
                click(simulation, "Shared counter: 2")?;
                click(simulation, "Open sibling window")?;
            }
            12 => {
                click(simulation, "rebuild parent (generation 0)")?;
                click(simulation, "rebuild parent (generation 1)")?;
            }
            13 => {
                click(simulation, "reverse (generation 0)")?;
                click(simulation, "reverse (generation 1)")?;
            }
            14 => {
                click(simulation, "edit paragraph 150 (0)")?;
                click(simulation, "edit paragraph 150 (1)")?;
            }
            _ => {}
        }

        exercise_scroll(simulation, index)?;
    }

    for &(x, y) in SCENARIO.probe_points {
        simulation.click_at(Offset::new(x, y))?;
    }
    simulation.press(Code::Tab)?;
    Ok(())
}

fn click(simulation: &Simulation, label: &str) -> Result<(), SimulationError> {
    simulation.click(label)?;
    example_support::wait_for_settled_frame(simulation)?;
    eprintln!("performance gallery: clicked {label:?}");
    Ok(())
}

fn exercise_scroll(simulation: &Simulation, scenario: usize) -> Result<(), SimulationError> {
    let target = Offset::new(720., 450.);
    simulation.move_mouse_to(target)?;
    let delta = if scenario == 2 {
        Offset::new(0., 5_000.)
    } else {
        Offset::new(0., 180.)
    };
    simulation.scroll(delta)?;
    example_support::wait_for_settled_frame(simulation)?;
    simulation.scroll(Offset::new(-delta.x, -delta.y))?;
    example_support::wait_for_settled_frame(simulation)
}
