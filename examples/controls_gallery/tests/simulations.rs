use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "controls_gallery",
    &[
        "Text fields",
        "Selection",
        "Buttons",
        "Tabs & sliders",
        "Progress & meter",
        "Toasts",
        "Cards & avatars",
        "Scroll areas",
        "Standard Button",
        "Primary Action",
        "Ghost Button",
    ],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 260.)),
    Some("qa"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
