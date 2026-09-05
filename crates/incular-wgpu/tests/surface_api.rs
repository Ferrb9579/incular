#[test]
fn surface_construction_requires_owned_live_targets() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/owned_surface_target.rs");
    cases.compile_fail("tests/ui/borrowed_surface_target.rs");
    cases.compile_fail("tests/ui/detached_surface_handles.rs");
}
