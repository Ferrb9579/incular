use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "scroll",
    &["Item 0"],
    &[(24., 24.), (160., 120.)],
    Some((0., 260.)),
    None,
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
