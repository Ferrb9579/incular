use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "flutter_widgets_gallery",
    &[],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 280.)),
    None,
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
