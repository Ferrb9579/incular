use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario =
    ExampleScenario::new("images", &[], &[(24., 24.), (180., 120.)], None, None);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
