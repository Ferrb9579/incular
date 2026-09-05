use serde_json::Value;

pub fn validate_member(row: &Value) -> Result<(), String> {
    let required = |field: &str| {
        row[field]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("missing {field}"))
    };
    let status = required("status")?;
    required("flutter_type")?;
    required("flutter_member")?;
    required("incular_type")?;
    required("incular_crate")?;
    let rationale = required("rationale")?;
    match status {
        "implemented" => {
            required("incular_member")?;
        }
        "rustified" | "merged" | "internal" => {
            required("incular_member")?;
            required("evidence")?;
            if rationale.len() < 20 {
                return Err("equivalent needs a specific rationale".into());
            }
        }
        "deferred" => {
            required("prerequisite")?;
            required("decision")?;
        }
        "omitted" => {
            required("decision")?;
        }
        _ => return Err(format!("unknown member status {status}")),
    }
    if matches!(status, "deferred" | "omitted") && rationale.len() < 20 {
        return Err("gap needs a specific rationale".into());
    }
    Ok(())
}
