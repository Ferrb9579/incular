#![allow(dead_code)]

#[path = "../main.rs"]
mod example;
#[test]
fn scenario_contract_is_valid() {
    crate::example::example_support::assert_scenario(crate::example::simulations::SCENARIO);
}
