use crate::example_support::{self, ExampleScenario};
use incular::testing::Simulation;

pub const SCENARIO: ExampleScenario = ExampleScenario::new(
    "performance_gallery",
    &[
        "100k widgets",
        "1M fixed list",
        "1M variable list",
        "large text",
        "many images",
        "many paths",
        "gradients",
        "effects",
        "nested scroll",
        "gesture stress",
        "transform anim",
        "multi-window",
        "Open static sibling window",
        "recon 10k",
        "keyed reorder",
        "doc edit",
    ],
    &[(24., 24.), (180., 120.), (320., 220.)],
    Some((0., 300.)),
    None,
);

pub fn run(simulation: Simulation) {
    example_support::run_smoke(simulation, SCENARIO);
}
