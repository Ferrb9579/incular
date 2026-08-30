use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "restoration",
    &[
        "Increment persistent counter",
        "Open restorable inspector",
        "Reset restoration for next launch",
        "Push restorable details",
        "Pop route",
        "Flush snapshot",
    ],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 280.)),
    Some("qa"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
