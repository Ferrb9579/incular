//! The packaged benchmark must use exactly the same workload as Electron.

#[test]
fn compressed_issue_dataset_matches_reference_bytes() {
    let decoded = miniz_oxide::inflate::decompress_to_vec_with_limit(
        include_bytes!("../../../examples/issue_tracker/issues.json.deflate"),
        1_048_576,
    )
    .expect("valid embedded asset within the application limit");
    assert_eq!(
        decoded.as_slice(),
        include_bytes!("../../../examples/issue_tracker/issues.json")
    );
    let issues: Vec<serde_json::Value> = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(issues.len(), 1_000);
}
