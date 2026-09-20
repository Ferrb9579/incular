//! Validate the reviewed architecture against Cargo's complete target graph.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("incular crate lives under <repo>/crates/incular")
        .to_path_buf()
}

fn contract() -> Value {
    serde_json::from_str(include_str!("../../../specs/architecture.json")).unwrap()
}

fn check_edges(policy: &Value, metadata: &Value) -> Result<(), String> {
    let rows = policy["packages"].as_array().unwrap();
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for package in metadata["packages"].as_array().unwrap() {
        let name = package["name"].as_str().unwrap();
        let Some(row) = rows.iter().find(|row| row["name"] == name) else {
            return Err(format!("unclassified workspace package {name}"));
        };
        let mut edges = Vec::new();
        for dependency in package["dependencies"].as_array().unwrap() {
            if dependency["kind"] == "dev" {
                continue;
            }
            let target = dependency["name"].as_str().unwrap();
            if target.starts_with("incular") {
                if !row["allowed_dependencies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|allowed| allowed == target)
                {
                    return Err(format!("unreviewed dependency {name} -> {target}"));
                }
                edges.push(target.to_owned());
            }
            if let Some(owners) = policy["external_boundaries"].get(target)
                && !owners.as_array().unwrap().iter().any(|owner| owner == name)
            {
                return Err(format!("external boundary violation {name} -> {target}"));
            }
        }
        graph.insert(name.to_owned(), edges);
    }
    // Check every target/optional edge together, including impossible feature
    // combinations: architectural layering must not rely on cfg to break cycles.
    fn visit<'a>(
        name: &'a str,
        graph: &'a BTreeMap<String, Vec<String>>,
        active: &mut Vec<&'a str>,
        done: &mut Vec<&'a str>,
    ) -> Result<(), String> {
        if active.contains(&name) {
            return Err(format!("dependency cycle through {name}"));
        }
        if done.contains(&name) {
            return Ok(());
        }
        active.push(name);
        for target in graph.get(name).into_iter().flatten() {
            visit(target, graph, active, done)?;
        }
        active.pop();
        done.push(name);
        Ok(())
    }
    let mut done = Vec::new();
    for name in graph.keys() {
        visit(name, &graph, &mut Vec::new(), &mut done)?;
    }
    Ok(())
}

#[test]
fn cargo_dependencies_respect_reviewed_boundaries() {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(repository_root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = serde_json::from_slice(&output.stdout).unwrap();
    check_edges(&contract(), &metadata).unwrap();
}

#[test]
fn every_workspace_package_has_one_documented_owner_api_class_and_evidence() {
    let root = repository_root();
    let policy = contract();
    assert_eq!(policy["schema_version"], 2);
    let rows = policy["packages"].as_array().unwrap();
    let metadata = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let workspace_names = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| package["name"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    let classes = ["application", "backend", "bridge"];
    let mut names = std::collections::BTreeSet::new();
    for row in rows {
        let name = row["name"].as_str().unwrap();
        assert!(names.insert(name), "duplicate package {name}");
        assert!(
            workspace_names.contains(name),
            "architecture row is not a workspace package: {name}"
        );
        let class = row["default_api_class"].as_str().unwrap();
        assert!(classes.contains(&class), "unknown API class for {name}");
        let owner = row["owner"].as_str().unwrap();
        let support = row["support"].as_str().unwrap();
        let path = row["path"].as_str().unwrap();
        let evidence = row["evidence"].as_str().unwrap();
        assert!(
            !owner.is_empty() && !support.is_empty() && !path.is_empty() && !evidence.is_empty()
        );
        assert!(
            root.join(path).join("Cargo.toml").exists(),
            "manifest path drift in {name}: {path}"
        );
        let evidence_path = root.join(evidence);
        assert!(
            evidence_path.is_file(),
            "missing support evidence for {name}: {evidence}"
        );
        let readme = fs::read_to_string(evidence_path).unwrap();
        assert!(
            readme.contains(&format!("| Ownership | {owner} |")),
            "owner drift in {name}"
        );
        assert!(
            readme.contains(&format!("| Support | {support} |")),
            "support drift in {name}"
        );
        assert!(
            readme.contains(&format!("| API class | {class};")),
            "API class drift in {name}"
        );
        assert!(
            agents.contains(&format!("- `{name}`: {owner}")),
            "AGENTS owner drift in {name}"
        );
    }
    assert_eq!(
        names, workspace_names,
        "architecture inventory must cover the complete workspace package set"
    );
    let mut overrides = std::collections::BTreeSet::new();
    for entry in policy["api_overrides"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        assert!(overrides.insert(path), "duplicate API override {path}");
        assert!(classes.contains(&entry["api_class"].as_str().unwrap()));
        let owner = path.split("::").next().unwrap().replace('_', "-");
        assert!(
            names.contains(owner.as_str()),
            "unknown override owner {owner}"
        );
    }
}

#[test]
fn boundary_guard_rejects_reverse_optional_native_and_cyclic_edges() {
    let policy = contract();
    let package = |name: &str, target: &str| {
        serde_json::json!({
            "name": name, "manifest_path": format!("/repo/crates/{name}/Cargo.toml"),
            "dependencies": [{"name": target, "kind": null, "optional": true,
                              "target": "cfg(target_os = \"android\")"}]
        })
    };
    for (owner, target) in [
        ("incular-core", "incular-widgets"),
        ("incular-wgpu", "incular-desktop"),
        ("incular-config", "winit"),
        ("incular-platform", "winit"),
        ("incular-platform", "raw-window-handle"),
    ] {
        let metadata = serde_json::json!({"packages": [package(owner, target)]});
        assert!(
            check_edges(&policy, &metadata).is_err(),
            "accepted {owner} -> {target}"
        );
    }
    let mut cyclic = policy.clone();
    let core = cyclic["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["name"] == "incular-core")
        .unwrap();
    core["allowed_dependencies"] = serde_json::json!(["incular-config"]);
    let metadata = serde_json::json!({"packages": [package("incular-core", "incular-config"), package("incular-config", "incular-core")]});
    assert!(
        check_edges(&cyclic, &metadata)
            .unwrap_err()
            .contains("cycle")
    );
}

#[test]
fn painting_facade_is_type_identical_to_rendering() {
    fn canonical(value: incular::painting::DisplayList) -> incular::rendering::DisplayList {
        value
    }
    let _ = canonical(incular::rendering::DisplayList::default());
}

#[test]
fn portable_runtime_and_widgets_do_not_pull_in_winit() {
    for package in ["incular-runtime", "incular-widgets"] {
        let output = Command::new(env!("CARGO"))
            .args([
                "tree",
                "--offline",
                "--package",
                package,
                "--edges",
                "normal,build",
                "--prefix",
                "none",
            ])
            .current_dir(repository_root())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let graph = String::from_utf8(output.stdout).unwrap();
        assert!(
            !graph.lines().any(|line| line.starts_with("winit ")),
            "{package} depends on Winit: {graph}"
        );
    }
}
