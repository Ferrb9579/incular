use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "workbench",
    &[
        "Increment",
        "Focus first",
        "Focus next",
        "Simulate Ctrl+S",
        "Undo",
        "Redo",
        "Validate",
        "Reset",
        "Push route",
        "Pop route",
    ],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 300.)),
    Some("QA"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
