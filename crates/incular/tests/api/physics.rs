use incular::prelude::*;
use std::time::Duration;

#[test]
fn test_simulations_and_physics_contract() {
    let spring = SpringDescription::with_duration_and_bounce(Duration::from_millis(500), 0.2);

    let sim = SpringSimulation::new(spring, 0.0, 300.0, 0.0, Tolerance::DEFAULT);
    assert!(!sim.is_done(Duration::ZERO));
    assert_eq!(sim.position(Duration::ZERO), 0.0);

    let friction = FrictionSimulation::new(0.05, 0.0, 500.0, Tolerance::DEFAULT);
    assert_eq!(friction.position(Duration::ZERO), 0.0);

    let gravity = GravitySimulation::new(980.0, 0.0, 1000.0, 0.0);
    assert_eq!(gravity.position(Duration::ZERO), 0.0);
}
