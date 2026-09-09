//! Reusable retained-property ledger validator, parameterized per family.
//!
//! Each family ledger records exported options with conversion, retained
//! owner, consumers, and named regressions. Validation resolves every
//! structured reference by parsing Rust syntax with `syn`, so comments
//! and string literals can never satisfy a reference, and every claimed
//! regression must be an exact `#[test]` function (helpers and prefix
//! matches fail). Option completeness is discovered from the family's
//! public API — see [`DiscoveryStyle`] — so an added or removed option
//! fails until the ledger is updated. Generated-builder coverage is an
//! explicit `builder_parity` test reference per struct, not a name
//! heuristic. A family ledger covers only its family; passing never
//! claims whole-codebase completeness, and resolving a test name never
//! proves its assertions establish a property contract.
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

/// Fixed consumer-phase vocabulary for the property-ledger schema. A
/// ledger selects applicable phases per record but cannot invent new
/// ones: unknown names fail even when listed in `allowed_phases`.
const FIXED_PHASES: &[&str] = &[
    "layout",
    "paint",
    "hit_test",
    "compositor",
    "animation",
    "semantics",
    "reconciliation",
    "lowering",
    "conversion",
];

/// How one struct's public options are discovered. Constructors that
/// build a whole configuration from parameters (like `Transform`) expose
/// options through those parameters; fluent builders expose them through
/// setter names. Only `pub` methods returning `Self` participate, so
/// getters and observers are never misclassified as options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryStyle {
    /// Option per setter name (minus `new`) plus `new` parameter names.
    /// Each ledger test binary compiles this module separately, so
    /// binaries without a fluent-builder family legitimately never
    /// construct this variant.
    #[allow(dead_code)]
    MethodNames,
    /// Option per parameter name of every `pub` `Self`-returning method.
    /// Only exercised by families with constructor-style APIs; each ledger
    /// test binary compiles this module separately, so binaries without
    /// such a family legitimately never construct this variant.
    #[allow(dead_code)]
    ConstructorParams,
}

/// One struct under ledger coverage: where it lives and how its options
/// are discovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructSpec {
    pub name: &'static str,
    pub file: &'static str,
    pub style: DiscoveryStyle,
}

/// One ledger family's source selection. Only the listed structs in the
/// listed files participate; everything else in those files is ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FamilySpec {
    pub structs: &'static [StructSpec],
}

/// One actionable validation failure. `validate_ledger` collects these
/// instead of panicking so malformed fixtures report every problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    UnsupportedSchema {
        found: String,
    },
    EmptyRecords,
    MissingAllowedPhases,
    UnsupportedBuilderAttribute {
        symbol: String,
        field: String,
        attribute: String,
    },
    UnknownDisposition {
        record: String,
        status: String,
    },
    DuplicateOption {
        symbol: String,
        option: String,
    },
    MissingOption {
        symbol: String,
        option: String,
    },
    StaleOption {
        symbol: String,
        option: String,
    },
    MissingBuilderCoverage {
        symbol: String,
    },
    UnknownPhase {
        record: String,
        phase: String,
    },
    MissingEvidence {
        record: String,
        field: &'static str,
    },
    UnresolvedReference {
        reference: String,
        reason: String,
    },
    RegressionNotATest {
        reference: String,
        reason: String,
    },
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
            Self::UnsupportedBuilderAttribute {
                symbol,
                field,
                attribute,
            } => write!(
                formatter,
                "{symbol}::{field}: unsupported builder attribute '{attribute}' \
                 (supported: default/default_code/doc/deprecated setters into, \
                 auto_into, strip_option, skip; renames via prefix/suffix and \
                 strip_bool/transform need resolver support)"
            ),
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
                "{symbol} derives a generated builder with no builder_parity reference"
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

/// How one struct field maps to generated builder API under
/// typed-builder 0.23.2 (verified against the installed macro source):
/// `setter(skip)` generates no setter; `into`/`auto_into`/`strip_option`
/// keep the field-named setter; `prefix`/`suffix` rename it and
/// `strip_bool`/`transform` change its shape, which this resolver does
/// not emulate. Any other `setter(...)` or `builder(...)` form is
/// rejected rather than silently assumed.
enum BuilderField {
    GeneratesSetter,
    Skipped,
}

fn builder_field_status(field: &syn::Field) -> Result<BuilderField, String> {
    let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
        return Err("tuple-struct fields have no setter name".to_owned());
    };
    let mut skipped = false;
    for attribute in &field.attrs {
        let syn::Meta::List(list) = &attribute.meta else {
            continue;
        };
        if !list.path.is_ident("builder") {
            continue;
        }
        let nested = list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .map_err(|_| "unparseable #[builder(...)]".to_owned())?;
        for meta in nested {
            match meta {
                syn::Meta::Path(path) => {
                    let Some(ident) = path.get_ident().map(ToString::to_string) else {
                        return Err("unsupported builder attribute form".to_owned());
                    };
                    match ident.as_str() {
                        "default" | "default_code" | "doc" | "deprecated" | "default_where" => {}
                        _ => return Err(ident),
                    }
                }
                syn::Meta::NameValue(named) => {
                    let Some(ident) = named.path.get_ident().map(ToString::to_string) else {
                        return Err("unsupported builder attribute form".to_owned());
                    };
                    match ident.as_str() {
                        "default" | "default_code" | "doc" | "deprecated" => {}
                        _ => return Err(ident),
                    }
                }
                syn::Meta::List(list) => {
                    let Some(ident) = list.path.get_ident().map(ToString::to_string) else {
                        return Err("unsupported builder attribute form".to_owned());
                    };
                    if ident != "setter" {
                        return Err(ident);
                    }
                    let inner = list
                        .parse_args_with(
                            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                        )
                        .map_err(|_| "unparseable setter(...)".to_owned())?;
                    for item in inner {
                        match item {
                            syn::Meta::Path(path) => {
                                let Some(ident) = path.get_ident().map(ToString::to_string) else {
                                    return Err("unsupported setter(...) form".to_owned());
                                };
                                match ident.as_str() {
                                    "into" | "auto_into" | "strip_option" | "doc"
                                    | "deprecated" => {}
                                    "skip" => skipped = true,
                                    _ => return Err(format!("setter({ident})")),
                                }
                            }
                            syn::Meta::NameValue(named) => {
                                let Some(ident) = named.path.get_ident().map(ToString::to_string)
                                else {
                                    return Err("unsupported setter(...) form".to_owned());
                                };
                                match ident.as_str() {
                                    "doc" | "deprecated" => {}
                                    _ => return Err(format!("setter({ident} = ..)")),
                                }
                            }
                            syn::Meta::List(_) => {
                                return Err("unsupported setter(...) form".to_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = name;
    if skipped {
        Ok(BuilderField::Skipped)
    } else {
        Ok(BuilderField::GeneratesSetter)
    }
}

fn returns_self(method: &syn::ImplItemFn) -> bool {
    match &method.sig.output {
        syn::ReturnType::Type(_, kind) => type_base_name(kind).as_deref() == Some("Self"),
        syn::ReturnType::Default => false,
    }
}

fn constructor_parameters(method: &syn::ImplItemFn) -> Vec<String> {
    method
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => match &*typed.pat {
                syn::Pat::Ident(named) => Some(named.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

/// Public options discovered from the source itself, as one set per
/// family: constructor inputs plus fluent options plus generated builder
/// options (private fields exposed through generated public builder
/// setters; skipped setters excluded; fields also exposed fluently
/// deduplicated). A struct deriving `TypedBuilder` must additionally name
/// an explicit `builder_parity` regression, resolved like any other test
/// reference — the validator checks the reference exists, while a
/// reviewer judges whether its assertions prove parity.
struct DiscoveredApi {
    options: BTreeSet<(String, String)>,
    builders: BTreeSet<String>,
}

enum DiscoveryFailure {
    Unreadable(String),
    UnsupportedAttribute {
        symbol: String,
        field: String,
        attribute: String,
    },
}

fn discover_struct(
    syntax: &syn::File,
    spec: &StructSpec,
    api: &mut DiscoveredApi,
) -> Result<(), DiscoveryFailure> {
    for item in &syntax.items {
        let syn::Item::Struct(item) = item else {
            continue;
        };
        if item.ident != spec.name {
            continue;
        }
        if has_typed_builder_derive(item) {
            api.builders.insert(spec.name.to_owned());
            if let syn::Fields::Named(fields) = &item.fields {
                for field in &fields.named {
                    let field_name = field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    match builder_field_status(field) {
                        Ok(BuilderField::GeneratesSetter) => {
                            api.options.insert((spec.name.to_owned(), field_name));
                        }
                        Ok(BuilderField::Skipped) => {}
                        Err(attribute) => {
                            return Err(DiscoveryFailure::UnsupportedAttribute {
                                symbol: spec.name.to_owned(),
                                field: field_name,
                                attribute,
                            });
                        }
                    }
                }
            }
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
        if self_type != spec.name {
            continue;
        }
        for member in &item.items {
            let syn::ImplItem::Fn(method) = member else {
                continue;
            };
            if !matches!(method.vis, syn::Visibility::Public(_)) {
                continue;
            }
            if !returns_self(method) {
                continue;
            }
            let name = method.sig.ident.to_string();
            match spec.style {
                DiscoveryStyle::MethodNames => {
                    if name == "new" {
                        for parameter in constructor_parameters(method) {
                            api.options.insert((self_type.clone(), parameter));
                        }
                    } else {
                        api.options.insert((self_type.clone(), name));
                    }
                }
                DiscoveryStyle::ConstructorParams => {
                    for parameter in constructor_parameters(method) {
                        api.options.insert((self_type.clone(), parameter));
                    }
                }
            }
        }
    }
    Ok(())
}

fn discover_api(root: &Path, family: &FamilySpec) -> Result<DiscoveredApi, DiscoveryFailure> {
    let mut parsed: std::collections::BTreeMap<&str, syn::File> = Default::default();
    for spec in family.structs {
        if !parsed.contains_key(spec.file) {
            let text = fs::read_to_string(root.join(spec.file))
                .map_err(|_| DiscoveryFailure::Unreadable(format!("cannot read {}", spec.file)))?;
            let syntax = syn::parse_file(&text)
                .map_err(|_| DiscoveryFailure::Unreadable(format!("cannot parse {}", spec.file)))?;
            parsed.insert(spec.file, syntax);
        }
    }
    let mut api = DiscoveredApi {
        options: BTreeSet::new(),
        builders: BTreeSet::new(),
    };
    for spec in family.structs {
        let syntax = parsed.get(spec.file).expect("file just parsed");
        discover_struct(syntax, spec, &mut api)?;
    }
    Ok(api)
}

/// Validate one ledger value against sources under `root` for one family,
/// collecting every failure instead of stopping at the first.
pub fn validate_ledger(
    ledger: &Value,
    root: &Path,
    family: &FamilySpec,
) -> Result<(), Vec<LedgerError>> {
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
    let allowed: BTreeSet<&str> = FIXED_PHASES.iter().copied().collect();
    match ledger["allowed_phases"].as_array() {
        None => errors.push(LedgerError::MissingAllowedPhases),
        Some(phases) => {
            if phases.is_empty() {
                errors.push(LedgerError::MissingAllowedPhases);
            }
            for phase in phases.iter().filter_map(Value::as_str) {
                if !allowed.contains(phase) {
                    errors.push(LedgerError::UnknownPhase {
                        record: "(allowed_phases)".to_owned(),
                        phase: phase.to_owned(),
                    });
                }
            }
        }
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
    // Family completeness: the source of truth is the family's public API,
    // discovered by parsing — never a second handwritten list. Both
    // directions fail: an undiscovered record is stale, and an unrecorded
    // public option is missing coverage.
    match discover_api(root, family) {
        Err(DiscoveryFailure::Unreadable(reason)) => {
            let files: BTreeSet<&str> = family.structs.iter().map(|spec| spec.file).collect();
            for file in files {
                errors.push(LedgerError::UnresolvedReference {
                    reference: file.to_owned(),
                    reason: reason.clone(),
                });
            }
        }
        Err(DiscoveryFailure::UnsupportedAttribute {
            symbol,
            field,
            attribute,
        }) => errors.push(LedgerError::UnsupportedBuilderAttribute {
            symbol,
            field,
            attribute,
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
            // Generated-builder coverage is an explicit per-struct
            // `builder_parity` regression reference, resolved through the
            // same exact-test resolver as every other regression. The
            // validator checks the reference exists; a reviewer judges
            // whether its assertions prove parity. A builder-named test
            // proves nothing by name alone.
            let parity = ledger["builder_parity"]
                .as_object()
                .cloned()
                .unwrap_or_default();
            for symbol in &api.builders {
                match parity.get(symbol) {
                    None => errors.push(LedgerError::MissingBuilderCoverage {
                        symbol: symbol.clone(),
                    }),
                    Some(regression) => {
                        if let Err(reason) = resolve_regression(&mut index, root, regression) {
                            errors.push(LedgerError::RegressionNotATest {
                                reference: format!(
                                    "builder_parity::{symbol}::{}",
                                    regression["name"].as_str().unwrap_or("?")
                                ),
                                reason,
                            });
                        }
                    }
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
