use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "base_ui_controls",
    &["Default", "Primary", "Ghost", "Overview", "Details"],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 220.)),
    Some("qa"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
