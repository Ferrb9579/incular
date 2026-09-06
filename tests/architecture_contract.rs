//! Validate the reviewed architecture against Cargo's complete target graph.
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path, process::Command};

fn contract() -> Value {
    serde_json::from_str(include_str!("../specs/architecture.json")).unwrap()
}

fn check_edges(policy: &Value, metadata: &Value) -> Result<(), String> {
    let rows = policy["crates"].as_array().unwrap();
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for package in metadata["packages"].as_array().unwrap() {
        let name = package["name"].as_str().unwrap();
        let Some(row) = rows.iter().find(|row| row["name"] == name) else {
            if package["manifest_path"]
                .as_str()
                .unwrap()
                .replace('\\', "/")
                .contains("/crates/")
            {
                return Err(format!("unclassified crate {name}"));
            }
            continue;
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
        .current_dir(env!("CARGO_MANIFEST_DIR"))
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
fn every_crate_has_one_documented_owner_and_api_class() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy = contract();
    assert_eq!(policy["schema_version"], 1);
    let rows = policy["crates"].as_array().unwrap();
    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    let classes = ["application", "backend", "bridge"];
    let mut names = std::collections::BTreeSet::new();
    for row in rows {
        let name = row["name"].as_str().unwrap();
        assert!(names.insert(name), "duplicate crate {name}");
        let class = row["default_api_class"].as_str().unwrap();
        assert!(classes.contains(&class), "unknown API class for {name}");
        let owner = row["owner"].as_str().unwrap();
        let support = row["support"].as_str().unwrap();
        assert!(!owner.is_empty() && !support.is_empty());
        let readme = fs::read_to_string(root.join("crates").join(name).join("README.md")).unwrap();
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
    let core = cyclic["crates"]
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
fn painting_stays_a_pure_type_identical_reexport() {
    let source = include_str!("../crates/incular-painting/src/lib.rs");
    let code = source
        .lines()
        .filter(|line| !line.trim().starts_with("//") && !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(code, ["pub use incular_rendering::*;"]);
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
            .current_dir(env!("CARGO_MANIFEST_DIR"))
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
