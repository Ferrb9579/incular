use serde_json::Value;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticOutcome {
    Supported,
    RustEquivalent,
    Internal,
    Omitted,
    Deferred,
}

pub fn semantic_outcome(status: &str) -> Result<SemanticOutcome, String> {
    match status {
        "implemented" | "IMPLEMENTED" | "EXACT" => Ok(SemanticOutcome::Supported),
        "rustified"
        | "RUSTIFIED"
        | "RUSTIFIED_IMPLEMENTED"
        | "merged"
        | "MERGED"
        | "MERGED_CORE"
        | "MERGED_CONTROLS"
        | "MERGED_CORE_IMPLEMENTED"
        | "MERGED_CONTROLS_IMPLEMENTED"
        | "ADAPTED" => Ok(SemanticOutcome::RustEquivalent),
        "internal" | "INTERNAL" => Ok(SemanticOutcome::Internal),
        "omitted"
        | "OMITTED"
        | "SKIPPED"
        | "SKIPPED_DART_MECHANIC"
        | "SKIPPED_DEPRECATED"
        | "SKIPPED_LEGACY"
        | "SKIPPED_PLATFORM" => Ok(SemanticOutcome::Omitted),
        "deferred" | "DEFERRED" | "DEFERRED_PLATFORM" | "DEFERRED_RENDERER" => {
            Ok(SemanticOutcome::Deferred)
        }
        _ => Err(format!("unknown parity status {status}")),
    }
}

pub fn validate_named_test_evidence(repo_root: &Path, evidence: &str) -> Result<(), String> {
    let mut checked = 0_usize;
    for reference in evidence
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let Some((path, function)) = reference.rsplit_once("::") else {
            return Err(format!("evidence must name a test function: {reference}"));
        };
        if function.is_empty() || path.is_empty() {
            return Err(format!("invalid evidence reference: {reference}"));
        }
        let path = repo_root.join(path);
        if !path.is_file() {
            return Err(format!("evidence file does not exist: {}", path.display()));
        }
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("unable to read {}: {error}", path.display()))?;
        let needle = format!("fn {function}(");
        if !source.contains(&needle) {
            return Err(format!(
                "evidence function {function} does not exist in {}",
                path.display()
            ));
        }
        checked += 1;
    }
    if checked == 0 {
        return Err("behavior evidence is empty".into());
    }
    Ok(())
}

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
    match semantic_outcome(status)? {
        SemanticOutcome::Supported | SemanticOutcome::RustEquivalent => {
            required("incular_member")?;
            required("evidence")?;
            if rationale.len() < 20 {
                return Err("equivalent needs a specific rationale".into());
            }
        }
        SemanticOutcome::Internal => {
            required("incular_member")?;
            required("evidence")?;
        }
        SemanticOutcome::Deferred => {
            required("prerequisite")?;
            required("decision")?;
        }
        SemanticOutcome::Omitted => {
            required("decision")?;
        }
    }
    if matches!(
        semantic_outcome(status)?,
        SemanticOutcome::Deferred | SemanticOutcome::Omitted
    ) && rationale.len() < 20
    {
        return Err("gap needs a specific rationale".into());
    }
    Ok(())
}
