use super::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "material_gallery",
    &[
        "Elevated",
        "Filled",
        "Outlined",
        "Text",
        "Action",
        "Advance",
        "Material list tile",
        "Menu",
        "New",
        "Menu",
        "Popup menu",
        "Popup menu",
        "SnackBar",
        "Dismiss",
        "Dialog",
    ],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 600.)),
    Some("qa"),
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
