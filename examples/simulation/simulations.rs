use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "simulation",
    &["Simulation input", "Increment"],
    &[(24., 24.), (180., 120.)],
    None,
    Some("QA"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
