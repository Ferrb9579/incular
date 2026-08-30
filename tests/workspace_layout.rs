use std::path::Path;

#[test]
fn canonical_examples_are_grouped_in_their_own_directories() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(root.join("examples/counter/main.rs").is_file());
    assert!(root.join("examples/counter/tests.rs").is_file());
    assert!(root.join("examples/counter/simulations.rs").is_file());
    assert!(root.join("examples/scroll/main.rs").is_file());
    assert!(root.join("examples/scroll/tests.rs").is_file());
    assert!(root.join("examples/scroll/simulations.rs").is_file());
    assert!(root.join("examples/studio/main.rs").is_file());
    assert!(root.join("examples/studio/tests.rs").is_file());
    assert!(root.join("examples/studio/simulations.rs").is_file());
    assert!(!root.join("examples/counter.rs").is_file());
    assert!(!root.join("examples/scroll.rs").is_file());
    assert!(!root.join("crates/incular/examples/counter.rs").is_file());
}
