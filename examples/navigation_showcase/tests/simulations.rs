use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "navigation_showcase",
    &[
        "Push fade + slide",
        "Show dialog",
        "Show sheet",
        "Pop / close overlay",
    ],
    &[(24., 24.), (180., 120.)],
    None,
    None,
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
