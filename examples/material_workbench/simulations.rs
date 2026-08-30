use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "material_workbench",
    &[
        "Open menu",
        "New document",
        "More",
        "Preferences",
        "Open menu",
        "Show snackbar",
        "Dismiss",
        "Open dialog",
        "Close dialog",
    ],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 260.)),
    Some("qa"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
