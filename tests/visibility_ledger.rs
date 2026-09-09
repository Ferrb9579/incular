//! Validate the Visibility retained-property ledger against the tree.
//!
//! The ledger (`specs/visibility_properties.json`) records every exported
//! Visibility/Offstage option with its conversion, retained owner,
//! consumers, and named regressions. Validation resolves every structured
//! reference by parsing Rust syntax with `syn`, so comments and string
//! literals can never satisfy a reference, and every claimed regression
//! must be an exact `#[test]` function (helpers and prefix matches fail).
//! Option completeness is discovered from the public API itself: an added
//! or removed setter fails until the ledger is updated. The ledger covers
//! only the Visibility family; passing here never claims whole-codebase
//! completeness, and resolving a test name never proves its assertions
//! establish a property contract — that remains human-reviewed.
//!
//! Reference shapes (all paths repository-relative):
//! - `{"path","kind":"struct"|"enum","name"}` — top-level item.
//! - `{"path","kind":"variant","owner","name"}` — enum variant.
//! - `{"path","kind":"function","name"}` — top-level free function.
//! - `{"path","kind":"method","owner","name"}` — method on an inherent or
//!   trait impl whose self type is `owner`.
//! - `{"path","kind":"trait_impl","trait","target"[, "source"]}` — an
//!   `impl Trait<Source> for Target` block.
//! - regressions use `{"path","name"[, "module"]}` — an exact `#[test]`
//!   function, optionally inside nested `module::path` modules.
//!
//! Unsupported forms (documented, rejected if attempted): methods provided
//! by a trait without a `trait_impl` reference, items reached only through
//! re-exports (reference the defining file), macro-generated items other
//! than the `TypedBuilder` builder covered by the completeness rule, and
//! struct fields (retention is asserted at the type level).
use serde_json::Value;
use std::{collections::BTreeSet, fmt, fs, path::Path, path::PathBuf};

/// One actionable validation failure. `validate_ledger` collects these
/// instead of panicking so malformed fixtures report every problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    UnsupportedSchema { found: String },
    EmptyRecords,
    MissingAllowedPhases,
    UnknownDisposition { record: String, status: String },
    DuplicateOption { symbol: String, option: String },
    MissingOption { symbol: String, option: String },
    StaleOption { symbol: String, option: String },
    MissingBuilderCoverage { symbol: String },
    UnknownPhase { record: String, phase: String },
    MissingEvidence { record: String, field: &'static str },
    UnresolvedReference { reference: String, reason: String },
    RegressionNotATest { reference: String, reason: String },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema { found } => {
                write!(formatter, "unsupported ledger schema_version {found}")
            }
            Self::EmptyRecords => write!(formatter, "ledger names no records"),
            Self::MissingAllowedPhases => {
                write!(formatter, "ledger has no nonempty allowed_phases")
            }
            Self::UnknownDisposition { record, status } => {
                write!(formatter, "{record}: unknown disposition {status}")
            }
            Self::DuplicateOption { symbol, option } => {
                write!(formatter, "duplicate ledger entry {symbol}::{option}")
            }
            Self::MissingOption { symbol, option } => write!(
                formatter,
                "{symbol}::{option} is public API without a ledger record"
            ),
            Self::StaleOption { symbol, option } => write!(
                formatter,
                "{symbol}::{option} has a ledger record but no such public API"
            ),
            Self::MissingBuilderCoverage { symbol } => write!(
                formatter,
                "{symbol} derives a generated builder with no builder-named regression"
            ),
            Self::UnknownPhase { record, phase } => {
                write!(formatter, "{record}: unknown consumer phase {phase}")
            }
            Self::MissingEvidence { record, field } => {
                write!(formatter, "implemented {record} names no {field}")
            }
            Self::UnresolvedReference { reference, reason } => {
                write!(formatter, "unresolved reference {reference}: {reason}")
            }
            Self::RegressionNotATest { reference, reason } => {
                write!(formatter, "regression {reference} is not a test: {reason}")
            }
        }
    }
}

/// Parsed view of one source file: top-level items plus inline modules.
/// Only real syntax counts; comments and string literals never appear here.
#[derive(Default)]
struct FileModel {
    structs: BTreeSet<String>,
    enums: std::collections::BTreeMap<String, Vec<String>>,
    free_functions: std::collections::BTreeMap<String, bool>,
    impls: Vec<ImplModel>,
    modules: std::collections::BTreeMap<String, FileModel>,
}

struct ImplModel {
    self_type: String,
    trait_name: Option<String>,
    trait_generics: Vec<String>,
    methods: Vec<String>,
}

fn type_base_name(kind: &syn::Type) -> Option<String> {
    if let syn::Type::Path(syn::TypePath { path, .. }) = kind
        && path.segments.len() == 1
    {
        return Some(path.segments[0].ident.to_string());
    }
    None
}

fn generic_type_names(path: &syn::Path) -> Vec<String> {
    let Some(segment) = path.segments.last() else {
        return Vec::new();
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Vec::new();
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(kind) => type_base_name(kind),
            _ => None,
        })
        .collect()
}

fn is_test_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("test")
}

fn has_typed_builder_derive(item: &syn::ItemStruct) -> bool {
    item.attrs.iter().any(|attribute| {
        attribute.path().is_ident("derive")
            && attribute.meta.require_list().is_ok_and(|list| {
                list.tokens
                    .to_string()
                    .split(|character: char| !character.is_alphanumeric() && character != '_')
                    .any(|token| token == "TypedBuilder")
            })
    })
}

fn collect_items(items: &[syn::Item], model: &mut FileModel) {
    for item in items {
        match item {
            syn::Item::Struct(item) => {
                model.structs.insert(item.ident.to_string());
            }
            syn::Item::Enum(item) => {
                model.enums.insert(
                    item.ident.to_string(),
                    item.variants
                        .iter()
                        .map(|variant| variant.ident.to_string())
                        .collect(),
                );
            }
            syn::Item::Fn(item) => {
                model.free_functions.insert(
                    item.sig.ident.to_string(),
                    item.attrs.iter().any(is_test_attribute),
                );
            }
            syn::Item::Impl(item) => {
                let Some(self_type) = type_base_name(&item.self_ty) else {
                    continue;
                };
                let (trait_name, trait_generics) = item
                    .trait_
                    .as_ref()
                    .map(|(_, path, _)| {
                        (
                            path.segments
                                .last()
                                .map(|segment| segment.ident.to_string()),
                            generic_type_names(path),
                        )
                    })
                    .unwrap_or((None, Vec::new()));
                model.impls.push(ImplModel {
                    self_type,
                    trait_name,
                    trait_generics,
                    methods: item
                        .items
                        .iter()
                        .filter_map(|member| match member {
                            syn::ImplItem::Fn(method) => Some(method.sig.ident.to_string()),
                            _ => None,
                        })
                        .collect(),
                });
            }
            syn::Item::Mod(module) => {
                if let Some((_, content)) = &module.content {
                    let mut nested = FileModel::default();
                    collect_items(content, &mut nested);
                    model.modules.insert(module.ident.to_string(), nested);
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct SourceIndex {
    files: std::collections::BTreeMap<PathBuf, (String, FileModel)>,
}

impl SourceIndex {
    fn model(&mut self, root: &Path, path: &str) -> Result<&FileModel, String> {
        let key = PathBuf::from(path);
        if !self.files.contains_key(&key) {
            let text = fs::read_to_string(root.join(&key))
                .map_err(|_| format!("cannot read file {path}"))?;
            let syntax = syn::parse_file(&text).map_err(|_| format!("cannot parse {path}"))?;
            let mut model = FileModel::default();
            collect_items(&syntax.items, &mut model);
            self.files.insert(key.clone(), (text, model));
        }
        Ok(&self.files[&key].1)
    }

    fn text(&mut self, root: &Path, path: &str) -> Result<String, String> {
        self.model(root, path)?;
        Ok(self.files[&PathBuf::from(path)].0.clone())
    }
}

fn render_reference(reference: &Value) -> String {
    let path = reference["path"].as_str().unwrap_or("?");
    let kind = reference["kind"].as_str().unwrap_or("?");
    let mut rendered = format!("{path}::{kind}");
    for field in ["owner", "trait", "target", "source", "name", "module"] {
        if let Some(value) = reference[field].as_str() {
            rendered.push_str(&format!("::{value}"));
        }
    }
    rendered
}

fn resolve_reference(
    index: &mut SourceIndex,
    root: &Path,
    reference: &Value,
) -> Result<(), String> {
    let path = reference["path"]
        .as_str()
        .ok_or_else(|| "reference has no path".to_owned())?;
    let kind = reference["kind"]
        .as_str()
        .ok_or_else(|| "reference has no kind".to_owned())?;
    let model = index.model(root, path)?;
    match kind {
        "struct" => {
            let name = reference["name"].as_str().unwrap_or_default();
            model.structs.contains(name).then_some(()).ok_or_else(|| {
                comment_or_missing(index, root, path, name, format!("no such struct '{name}'"))
            })
        }
        "enum" => {
            let name = reference["name"].as_str().unwrap_or_default();
            model.enums.contains_key(name).then_some(()).ok_or_else(|| {
                comment_or_missing(index, root, path, name, format!("no such enum '{name}'"))
            })
        }
        "variant" => {
            let owner = reference["owner"].as_str().unwrap_or_default();
            let name = reference["name"].as_str().unwrap_or_default();
            match model.enums.get(owner) {
                None => Err(comment_or_missing(
                    index,
                    root,
                    path,
                    owner,
                    format!("no such enum '{owner}'"),
                )),
                Some(variants) if variants.contains(&name.to_owned()) => Ok(()),
                Some(_) => Err(format!("enum '{owner}' has no variant '{name}'")),
            }
        }
        "function" => {
            let name = reference["name"].as_str().unwrap_or_default();
            model
                .free_functions
                .contains_key(name)
                .then_some(())
                .ok_or_else(|| {
                    comment_or_missing(
                        index,
                        root,
                        path,
                        name,
                        format!("no such free function '{name}'"),
                    )
                })
        }
        "method" => {
            let owner = reference["owner"].as_str().unwrap_or_default();
            let name = reference["name"].as_str().unwrap_or_default();
            let candidates: Vec<&ImplModel> = model
                .impls
                .iter()
                .filter(|candidate| candidate.self_type == owner)
                .collect();
            if candidates.is_empty() {
                return Err(comment_or_missing(
                    index,
                    root,
                    path,
                    owner,
                    format!("no impl block for owner '{owner}'"),
                ));
            }
            if let Some(expected) = reference["trait"].as_str()
                && !candidates.iter().any(|candidate| {
                    candidate.trait_name.as_deref() == Some(expected)
                        && candidate.methods.iter().any(|method| method == name)
                })
            {
                return Err(format!("no trait method '{expected}::{name}' on '{owner}'"));
            }
            candidates
                .iter()
                .any(|candidate| candidate.methods.iter().any(|method| method == name))
                .then_some(())
                .ok_or_else(|| format!("no method '{name}' on owner '{owner}'"))
        }
        "trait_impl" => {
            let expected_trait = reference["trait"].as_str().unwrap_or_default();
            let target = reference["target"].as_str().unwrap_or_default();
            let source = reference["source"].as_str().unwrap_or_default();
            let mut saw_trait = false;
            let mut saw_target = false;
            let mut saw_pair = false;
            for candidate in &model.impls {
                let Some(actual_trait) = candidate.trait_name.as_deref() else {
                    continue;
                };
                saw_trait |= actual_trait == expected_trait;
                saw_target |= candidate.self_type == target;
                if actual_trait != expected_trait || candidate.self_type != target {
                    continue;
                }
                saw_pair = true;
                if source.is_empty()
                    || candidate
                        .trait_generics
                        .iter()
                        .any(|generic| generic == source)
                {
                    return Ok(());
                }
            }
            Err(if !saw_trait {
                format!("no impl of trait '{expected_trait}' in file")
            } else if !saw_target {
                format!("no trait impl with target type '{target}'")
            } else if saw_pair {
                format!("impl {expected_trait} for {target} names no source '{source}'")
            } else {
                format!("no impl {expected_trait} for {target}")
            })
        }
        other => Err(format!("unsupported reference kind '{other}'")),
    }
}

/// Distinguish a genuinely absent symbol from one that only appears in
/// comments or string literals (which syntax parsing ignores by design).
fn comment_or_missing(
    index: &mut SourceIndex,
    root: &Path,
    path: &str,
    name: &str,
    missing: String,
) -> String {
    if name.is_empty() {
        return missing;
    }
    match index.text(root, path) {
        Ok(text) if text.contains(name) => {
            format!("'{name}' appears only in comments or string literals; no such item")
        }
        _ => missing,
    }
}

fn resolve_regression(
    index: &mut SourceIndex,
    root: &Path,
    regression: &Value,
) -> Result<(), String> {
    let path = regression["path"]
        .as_str()
        .ok_or_else(|| "regression has no path".to_owned())?;
    let name = regression["name"]
        .as_str()
        .ok_or_else(|| "regression has no name".to_owned())?;
    let model = index.model(root, path)?;
    let mut scope = model;
    if let Some(modules) = regression["module"].as_str() {
        for module in modules.split("::") {
            scope = scope
                .modules
                .get(module)
                .ok_or_else(|| format!("no module '{module}' in {path}"))?;
        }
    }
    match scope.free_functions.get(name) {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "function '{name}' exists but carries no #[test] attribute"
        )),
        None => {
            let near: Vec<&String> = scope
                .free_functions
                .keys()
                .filter(|candidate| {
                    candidate.starts_with(name) || name.starts_with(candidate.as_str())
                })
                .collect();
            if near.is_empty() {
                Err(comment_or_missing(
                    index,
                    root,
                    path,
                    name,
                    format!("no function '{name}' in {path}"),
                ))
            } else {
                Err(format!(
                    "no exact match for '{name}'; prefix matches do not count (nearest: {})",
                    near.iter()
                        .map(|candidate| candidate.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }
}

/// Public options discovered from the source itself: every `pub fn` in the
/// inherent impl of each target struct, minus constructors, plus each
/// constructor's parameter names. The generated `builder()` API is covered
/// separately: a struct deriving `TypedBuilder` must have a builder-named
/// regression proving builder/fluent parity.
struct DiscoveredApi {
    options: BTreeSet<(String, String)>,
    builders: BTreeSet<String>,
}

fn discover_api(root: &Path, path: &str) -> Result<DiscoveredApi, String> {
    let text = fs::read_to_string(root.join(path)).map_err(|_| format!("cannot read {path}"))?;
    let syntax = syn::parse_file(&text).map_err(|_| format!("cannot parse {path}"))?;
    let mut api = DiscoveredApi {
        options: BTreeSet::new(),
        builders: BTreeSet::new(),
    };
    for item in &syntax.items {
        let syn::Item::Struct(item) = item else {
            continue;
        };
        let symbol = item.ident.to_string();
        if !matches!(symbol.as_str(), "Visibility" | "Offstage") {
            continue;
        }
        if has_typed_builder_derive(item) {
            api.builders.insert(symbol.clone());
        }
    }
    for item in &syntax.items {
        let syn::Item::Impl(item) = item else {
            continue;
        };
        if item.trait_.is_some() {
            continue;
        }
        let Some(self_type) = type_base_name(&item.self_ty) else {
            continue;
        };
        if !matches!(self_type.as_str(), "Visibility" | "Offstage") {
            continue;
        }
        for member in &item.items {
            let syn::ImplItem::Fn(method) = member else {
                continue;
            };
            if !matches!(method.vis, syn::Visibility::Public(_)) {
                continue;
            }
            let name = method.sig.ident.to_string();
            if name == "new" {
                for input in &method.sig.inputs {
                    if let syn::FnArg::Typed(typed) = input
                        && let syn::Pat::Ident(named) = &*typed.pat
                    {
                        api.options
                            .insert((self_type.clone(), named.ident.to_string()));
                    }
                }
            } else {
                api.options.insert((self_type.clone(), name));
            }
        }
    }
    Ok(api)
}

/// Validate one ledger value against sources under `root`, collecting every
/// failure instead of stopping at the first.
pub fn validate_ledger(ledger: &Value, root: &Path) -> Result<(), Vec<LedgerError>> {
    let mut errors = Vec::new();
    if ledger["schema_version"] != 2 {
        errors.push(LedgerError::UnsupportedSchema {
            found: ledger["schema_version"].to_string(),
        });
        return Err(errors);
    }
    let Some(records) = ledger["records"].as_array() else {
        errors.push(LedgerError::EmptyRecords);
        return Err(errors);
    };
    if records.is_empty() {
        errors.push(LedgerError::EmptyRecords);
        return Err(errors);
    }
    let allowed: BTreeSet<&str> = ledger["allowed_phases"]
        .as_array()
        .map(|phases| phases.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if allowed.is_empty() {
        errors.push(LedgerError::MissingAllowedPhases);
    }
    let mut index = SourceIndex::default();
    let mut seen = BTreeSet::new();
    for record in records {
        let symbol = record["symbol"].as_str().unwrap_or("?");
        let option = record["option"].as_str().unwrap_or("?");
        let context = format!("{symbol}::{option}");
        if !seen.insert((symbol.to_owned(), option.to_owned())) {
            errors.push(LedgerError::DuplicateOption {
                symbol: symbol.to_owned(),
                option: option.to_owned(),
            });
        }
        let status = record["status"].as_str().unwrap_or_default();
        if !["implemented", "intentionally_unsupported", "unresolved"].contains(&status) {
            errors.push(LedgerError::UnknownDisposition {
                record: context.clone(),
                status: status.to_owned(),
            });
            continue;
        }
        if status == "implemented" {
            if record["regressions"].as_array().is_none_or(Vec::is_empty) {
                errors.push(LedgerError::MissingEvidence {
                    record: context.clone(),
                    field: "regressions",
                });
            }
            if record["consumers"]
                .as_object()
                .is_none_or(|map| map.is_empty())
            {
                errors.push(LedgerError::MissingEvidence {
                    record: context.clone(),
                    field: "consumers",
                });
            }
        }
        for (phase, targets) in record["consumers"].as_object().cloned().unwrap_or_default() {
            if !allowed.contains(phase.as_str()) {
                errors.push(LedgerError::UnknownPhase {
                    record: context.clone(),
                    phase: phase.clone(),
                });
            }
            for target in targets.as_array().cloned().unwrap_or_default() {
                if let Err(reason) = resolve_reference(&mut index, root, &target) {
                    errors.push(LedgerError::UnresolvedReference {
                        reference: render_reference(&target),
                        reason,
                    });
                }
            }
        }
        for field in ["conversion", "retained_owner"] {
            if let Some(target) = record.get(field)
                && let Err(reason) = resolve_reference(&mut index, root, target)
            {
                errors.push(LedgerError::UnresolvedReference {
                    reference: render_reference(target),
                    reason,
                });
            }
        }
        if let Some(comparison) = record.get("comparison") {
            for target in comparison["refs"].as_array().cloned().unwrap_or_default() {
                if let Err(reason) = resolve_reference(&mut index, root, &target) {
                    errors.push(LedgerError::UnresolvedReference {
                        reference: render_reference(&target),
                        reason,
                    });
                }
            }
        }
        for regression in record["regressions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if let Err(reason) = resolve_regression(&mut index, root, &regression) {
                let reference = format!(
                    "{}::{}",
                    regression["path"].as_str().unwrap_or("?"),
                    regression["name"].as_str().unwrap_or("?")
                );
                errors.push(LedgerError::RegressionNotATest { reference, reason });
            }
        }
    }
    // Family completeness: the source of truth is the public API in
    // visibility.rs, discovered by parsing — never a second handwritten
    // list. Both directions fail: an undiscovered record is stale, and an
    // unrecorded public option is missing coverage.
    match discover_api(
        root,
        "crates/incular-widgets/src/layout/basic/visibility.rs",
    ) {
        Err(reason) => errors.push(LedgerError::UnresolvedReference {
            reference: "crates/incular-widgets/src/layout/basic/visibility.rs".to_owned(),
            reason,
        }),
        Ok(api) => {
            let ledgered: BTreeSet<(String, String)> = records
                .iter()
                .map(|record| {
                    (
                        record["symbol"].as_str().unwrap_or_default().to_owned(),
                        record["option"].as_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect();
            for (symbol, option) in &api.options {
                if !ledgered.contains(&(symbol.clone(), option.clone())) {
                    errors.push(LedgerError::MissingOption {
                        symbol: symbol.clone(),
                        option: option.clone(),
                    });
                }
            }
            for (symbol, option) in &ledgered {
                if !api.options.contains(&(symbol.clone(), option.clone())) {
                    errors.push(LedgerError::StaleOption {
                        symbol: symbol.clone(),
                        option: option.clone(),
                    });
                }
            }
            for symbol in &api.builders {
                let covered = records
                    .iter()
                    .filter(|record| record["symbol"].as_str() == Some(symbol))
                    .flat_map(|record| {
                        record["regressions"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                    })
                    .any(|regression| {
                        regression["name"]
                            .as_str()
                            .is_some_and(|name| name.contains("builder"))
                    });
                if !covered {
                    errors.push(LedgerError::MissingBuilderCoverage {
                        symbol: symbol.clone(),
                    });
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn real_ledger() -> Value {
    serde_json::from_str(include_str!("../specs/visibility_properties.json")).unwrap()
}

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

#[test]
fn visibility_ledger_validates() {
    if let Err(errors) = validate_ledger(&real_ledger(), &repository_root()) {
        panic!(
            "visibility ledger invalid:\n{}",
            errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Unique scratch root per test so parallel cases never share fixtures.
fn fixture_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "incular-ledger-fixture-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("crates/fixture")).unwrap();
    root
}

fn write_source(root: &Path, name: &str, text: &str) {
    std::fs::write(root.join("crates/fixture").join(name), text).unwrap();
}

/// Minimal well-formed ledger over fixture sources: a miniature
/// `visibility.rs` with one struct, two options, and a generated builder,
/// plus one consumer and two regressions. Negative cases mutate a copy.
/// `syn` parses syntax only, so undefined types in fixtures are fine.
fn fixture_ledger() -> Value {
    serde_json::json!({
        "schema_version": 2,
        "family": "Fixture",
        "scope": "validator fixtures only",
        "allowed_phases": ["layout"],
        "records": [
            {
                "symbol": "Visibility",
                "option": "visible",
                "default": true,
                "validation": "bool setter",
                "conversion": {
                    "path": "crates/fixture/convert.rs",
                    "kind": "trait_impl",
                    "trait": "From",
                    "target": "Widget",
                    "source": "Visibility"
                },
                "retained_owner": {
                    "path": "crates/fixture/owner.rs",
                    "kind": "struct",
                    "name": "FixturePolicy"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "layout": [
                        {
                            "path": "crates/fixture/owner.rs",
                            "kind": "method",
                            "owner": "WidgetTree",
                            "name": "layout_fixture"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_renders"},
                    {"path": "crates/fixture/tests.rs", "name": "fixture_builder_parity"}
                ],
                "status": "implemented"
            },
            {
                "symbol": "Visibility",
                "option": "child",
                "default": "required, no default",
                "validation": "required conversion parameter",
                "conversion": {
                    "path": "crates/fixture/convert.rs",
                    "kind": "trait_impl",
                    "trait": "From",
                    "target": "Widget",
                    "source": "Visibility"
                },
                "retained_owner": {
                    "path": "crates/fixture/owner.rs",
                    "kind": "struct",
                    "name": "FixturePolicy"
                },
                "comparison": {"prose": "none", "refs": []},
                "consumers": {
                    "layout": [
                        {
                            "path": "crates/fixture/owner.rs",
                            "kind": "method",
                            "owner": "WidgetTree",
                            "name": "layout_fixture"
                        }
                    ]
                },
                "regressions": [
                    {"path": "crates/fixture/tests.rs", "name": "fixture_renders"}
                ],
                "status": "implemented"
            }
        ]
    })
}

fn fixture_sources(root: &Path) {
    // Completeness discovers options from this exact path, so fixtures
    // carry a miniature one whose public API matches the fixture ledger
    // exactly: `new(child)` plus the `visible` setter, with a generated
    // builder covered by the builder-named regression.
    std::fs::create_dir_all(root.join("crates/incular-widgets/src/layout/basic")).unwrap();
    std::fs::write(
        root.join("crates/incular-widgets/src/layout/basic/visibility.rs"),
        "#[derive(Clone, TypedBuilder)]\npub struct Visibility {\n    visible: bool,\n    child: Widget,\n}\npub struct Widget;\nimpl Visibility {\n    pub fn new(child: impl Into<Widget>) -> Self {\n        let _ = child;\n        unimplemented!()\n    }\n    pub fn visible(mut self, visible: bool) -> Self {\n        let _ = visible;\n        unimplemented!()\n    }\n}\n",
    )
    .unwrap();
    write_source(
        root,
        "convert.rs",
        "pub struct Visibility;\npub struct Widget;\nimpl From<Visibility> for Widget {\n    fn from(_: Visibility) -> Self { Widget }\n}\n",
    );
    write_source(
        root,
        "owner.rs",
        "pub struct FixturePolicy;\npub struct WidgetTree;\nimpl WidgetTree {\n    pub fn layout_fixture(&self) {}\n}\n",
    );
    write_source(
        root,
        "tests.rs",
        "#[test]\nfn fixture_renders() {}\n#[test]\nfn fixture_builder_parity() {}\nfn fixture_helper() {}\n#[test]\nfn fixture_renders_extended() {}\n",
    );
}

fn errors_for(ledger: &Value, root: &Path) -> Vec<LedgerError> {
    validate_ledger(ledger, root).expect_err("fixture must be rejected")
}

#[test]
fn validator_accepts_a_well_formed_fixture() {
    let root = fixture_root("positive");
    fixture_sources(&root);
    if let Err(errors) = validate_ledger(&fixture_ledger(), &root) {
        panic!("well-formed fixture must validate: {errors:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_empty_records_and_missing_options() {
    let root = fixture_root("empty");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["records"] = Value::Array(Vec::new());
    let errors = errors_for(&ledger, &root);
    assert!(errors.contains(&LedgerError::EmptyRecords), "{errors:?}");

    // A ledger without the `visible` record misses discovered public API.
    let mut ledger = fixture_ledger();
    let records = ledger["records"].as_array_mut().unwrap();
    let position = records
        .iter()
        .position(|record| record["option"] == "visible")
        .unwrap();
    records.remove(position);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::MissingOption {
            symbol: "Visibility".to_owned(),
            option: "visible".to_owned(),
        }),
        "{errors:?}"
    );

    // A record for a removed option is stale.
    let mut ledger = fixture_ledger();
    ledger["records"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "symbol": "Visibility",
            "option": "removed_option",
            "default": false,
            "validation": "gone",
            "conversion": {
                "path": "crates/fixture/convert.rs",
                "kind": "trait_impl",
                "trait": "From",
                "target": "Widget",
                "source": "Visibility"
            },
            "retained_owner": {
                "path": "crates/fixture/owner.rs",
                "kind": "struct",
                "name": "FixturePolicy"
            },
            "comparison": {"prose": "none", "refs": []},
            "consumers": {
                "layout": [
                    {
                        "path": "crates/fixture/owner.rs",
                        "kind": "method",
                        "owner": "WidgetTree",
                        "name": "layout_fixture"
                    }
                ]
            },
            "regressions": [
                {"path": "crates/fixture/tests.rs", "name": "fixture_renders"}
            ],
            "status": "implemented"
        }));
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::StaleOption {
            symbol: "Visibility".to_owned(),
            option: "removed_option".to_owned(),
        }),
        "{errors:?}"
    );

    // A derived builder without builder-named regression evidence fails.
    let mut ledger = fixture_ledger();
    for record in ledger["records"].as_array_mut().unwrap() {
        record["regressions"] = serde_json::json!([
            {"path": "crates/fixture/tests.rs", "name": "fixture_renders"}
        ]);
    }
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::MissingBuilderCoverage {
            symbol: "Visibility".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_duplicate_options_and_unknown_phases() {
    let root = fixture_root("duplicates");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    let duplicate = ledger["records"][0].clone();
    ledger["records"].as_array_mut().unwrap().push(duplicate);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::DuplicateOption {
            symbol: "Visibility".to_owned(),
            option: "visible".to_owned(),
        }),
        "{errors:?}"
    );

    let mut ledger = fixture_ledger();
    ledger["records"][0]["consumers"] = serde_json::json!({
        "teleport": [
            {
                "path": "crates/fixture/owner.rs",
                "kind": "method",
                "owner": "WidgetTree",
                "name": "layout_fixture"
            }
        ]
    });
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.contains(&LedgerError::UnknownPhase {
            record: "Visibility::visible".to_owned(),
            phase: "teleport".to_owned(),
        }),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_missing_methods_and_wrong_trait_targets() {
    let root = fixture_root("refs");
    fixture_sources(&root);
    let mut ledger = fixture_ledger();
    ledger["records"][0]["consumers"]["layout"][0]["name"] =
        Value::String("no_such_method".to_owned());
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnresolvedReference { reason, .. }
            if reason.contains("no method 'no_such_method' on owner 'WidgetTree'")
        )),
        "{errors:?}"
    );

    // Same file and trait, but no impl targets Widget through Fixture.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["conversion"]["source"] = Value::String("Other".to_owned());
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnresolvedReference { reason, .. }
            if reason.contains("no source 'Other'")
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_comment_only_symbols_and_plain_helpers() {
    let root = fixture_root("comments");
    fixture_sources(&root);
    write_source(
        &root,
        "comments.rs",
        "// GhostPolicy documents a removed type.\nconst NOTE: &str = \"GhostPolicy\";\n",
    );
    let mut ledger = fixture_ledger();
    ledger["records"][0]["retained_owner"] = serde_json::json!({
        "path": "crates/fixture/comments.rs",
        "kind": "struct",
        "name": "GhostPolicy"
    });
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::UnresolvedReference { reason, .. }
            if reason.contains("only in comments or string literals")
        )),
        "{errors:?}"
    );

    // A helper without #[test] is not regression coverage.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["regressions"] = serde_json::json!([
        {"path": "crates/fixture/tests.rs", "name": "fixture_helper"}
    ]);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::RegressionNotATest { reason, .. }
            if reason.contains("carries no #[test] attribute")
        )),
        "{errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn validator_rejects_prefix_only_test_matches_and_resolves_nested_modules() {
    let root = fixture_root("prefix");
    fixture_sources(&root);
    write_source(
        &root,
        "nested.rs",
        "mod inner {\n    #[test]\n    fn nested_works() {}\n}\n",
    );
    // fixture_renders_extended exists; fixture_renders must not match it.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["regressions"] = serde_json::json!([
        {"path": "crates/fixture/tests.rs", "name": "fixture_renders_missing"}
    ]);
    let errors = errors_for(&ledger, &root);
    assert!(
        errors.iter().any(|error| matches!(
            error,
            LedgerError::RegressionNotATest { reason, .. }
            if reason.contains("no exact match")
        )),
        "{errors:?}"
    );

    // Nested modules resolve through the module path.
    let mut ledger = fixture_ledger();
    ledger["records"][0]["regressions"] = serde_json::json!([
        {"path": "crates/fixture/nested.rs", "name": "nested_works", "module": "inner"}
    ]);
    let nested_path = root.join("crates/fixture/nested.rs");
    assert!(nested_path.exists());
    let errors = validate_ledger(&ledger, &root).expect_err("stale fixture record remains");
    assert!(
        !errors
            .iter()
            .any(|error| matches!(error, LedgerError::RegressionNotATest { .. })),
        "nested test must resolve: {errors:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
