use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "scrolling_complete",
    &[],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 320.)),
    None,
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
