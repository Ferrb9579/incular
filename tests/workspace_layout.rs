use std::path::Path;

#[test]
fn canonical_examples_are_at_the_workspace_root() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(root.join("examples/counter.rs").is_file());
    assert!(root.join("examples/scroll.rs").is_file());
    assert!(!root.join("crates/incular/examples/counter.rs").is_file());
}
