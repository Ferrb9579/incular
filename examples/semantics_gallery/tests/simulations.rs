use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "semantics_gallery",
    &["Save", "Dismiss"],
    &[(24., 24.), (180., 120.)],
    Some((0., 260.)),
    Some("qa"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
