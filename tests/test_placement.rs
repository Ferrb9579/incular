use std::fs;
use std::path::Path;

#[test]
fn test_code_is_kept_under_tests_directories() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();

    for root in ["crates", "tools", "examples"] {
        collect_rust_files(&repository.join(root), repository, &mut violations);
    }

    violations.sort();
    if !violations.is_empty() {
        panic!(
            "test code must live under a tests/ directory:\n{}",
            violations.join("\n")
        );
    }
}

fn collect_rust_files(directory: &Path, repository: &Path, violations: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, repository, violations);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }

        let relative = path.strip_prefix(repository).unwrap_or(&path);
        if has_tests_component(relative) {
            continue;
        }

        let relative_display = relative.display().to_string();
        if is_legacy_example_test_file(relative) {
            violations.push(format!(
                "{relative_display}: move the file into the example's tests/ directory"
            ));
        }

        let Ok(source) = fs::read_to_string(&path) else {
            violations.push(format!("{relative_display}: could not read Rust source"));
            continue;
        };

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            let has_test_attribute = (trimmed.starts_with("#[cfg(") && trimmed.contains("test"))
                || trimmed.starts_with("#[test]")
                || trimmed.starts_with("#[bench]");
            let has_test_module = is_test_module_declaration(trimmed);
            if has_test_attribute || has_test_module {
                violations.push(format!(
                    "{relative_display}:{}: move test code into a tests/ directory",
                    line_number + 1
                ));
            }
        }
    }
}

fn has_tests_component(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "tests")
}

fn is_legacy_example_test_file(path: &Path) -> bool {
    let components: Vec<_> = path.components().collect();
    components.len() == 3
        && components[0].as_os_str() == "examples"
        && matches!(
            components[2].as_os_str().to_str(),
            Some("tests.rs" | "example_tests.rs" | "simulations.rs")
        )
}

fn is_test_module_declaration(line: &str) -> bool {
    let Some(rest) = line
        .strip_prefix("mod ")
        .or_else(|| line.strip_prefix("pub mod "))
        .or_else(|| line.strip_prefix("pub(crate) mod "))
    else {
        return false;
    };

    let name = rest
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    name == "test"
        || name == "tests"
        || name.starts_with("test_")
        || name.ends_with("_test")
        || name.ends_with("_tests")
}
