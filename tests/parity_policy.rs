#[path = "support/parity_status.rs"]
mod parity_status;

#[test]
fn explicit_deviations_require_an_owner_reason_and_evidence() {
    let row = serde_json::json!({"flutter_type":"Example", "flutter_member":"member",
        "incular_type":"Example", "incular_crate":"incular-widgets", "incular_member":"equivalent",
        "status":"rustified", "rationale":"An owned Rust value replaces a Dart lifecycle object.",
        "evidence":"tests/widgets_3471_api.rs"});
    parity_status::validate_member(&row).unwrap();
    for field in ["evidence", "rationale", "incular_crate"] {
        let mut invalid = row.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(parity_status::validate_member(&invalid).is_err());
    }
    let mut gap = row.clone();
    gap["status"] = "deferred".into();
    assert!(parity_status::validate_member(&gap).is_err());
    gap["decision"] = "B08".into();
    gap["prerequisite"] = "Native host integration".into();
    parity_status::validate_member(&gap).unwrap();
    gap["status"] = "omitted".into();
    parity_status::validate_member(&gap).unwrap();
    gap["status"] = "unknown".into();
    assert!(parity_status::validate_member(&gap).is_err());
}

#[test]
fn semantic_statuses_normalize_without_turning_port_policy_into_support() {
    use parity_status::{SemanticOutcome, semantic_outcome};

    assert_eq!(
        semantic_outcome("IMPLEMENTED").unwrap(),
        SemanticOutcome::Supported
    );
    assert_eq!(
        semantic_outcome("RUSTIFIED_IMPLEMENTED").unwrap(),
        SemanticOutcome::RustEquivalent
    );
    assert_eq!(
        semantic_outcome("MERGED_CONTROLS_IMPLEMENTED").unwrap(),
        SemanticOutcome::RustEquivalent
    );
    assert_eq!(
        semantic_outcome("DEFERRED_RENDERER").unwrap(),
        SemanticOutcome::Deferred
    );
    assert_eq!(
        semantic_outcome("SKIPPED_DART_MECHANIC").unwrap(),
        SemanticOutcome::Omitted
    );
    assert!(semantic_outcome("RUSTIFIED_PORT_POLICY_ONLY").is_err());
}

#[test]
fn behavior_evidence_must_resolve_to_a_named_test() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    parity_status::validate_named_test_evidence(
        root,
        "tests/api/text.rs::test_text_and_style_compile_contract",
    )
    .unwrap();
    assert!(
        parity_status::validate_named_test_evidence(
            root,
            "tests/api/text.rs::missing_behavior_test"
        )
        .is_err()
    );
}
