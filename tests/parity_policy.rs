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
