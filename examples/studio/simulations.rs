use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "studio",
    &[],
    &[(24., 24.), (180., 120.), (420., 260.)],
    Some((0., 360.)),
    Some("QA"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
