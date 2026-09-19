//! W7 qualified public-surface discovery.
//!
//! This intentionally lives beside, rather than inside, the schema-v2 W3
//! validator. W7 needs qualified same-name identities, public fields and the
//! complete generated-builder surface; changing W3's schema would create churn
//! in 27 already-closed ledgers.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PublicOption {
    symbol: String,
    option: String,
    entrypoints: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PublicSymbol {
    qualified: String,
    kind: &'static str,
}

fn crate_module(crate_name: &str, src: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(src).expect("source file under src");
    let mut components = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let file = components.pop().unwrap_or_default();
    let stem = file.strip_suffix(".rs").unwrap_or(&file);
    if stem != "lib" && stem != "mod" {
        components.push(stem.to_owned());
    }
    if components.is_empty() {
        crate_name.to_owned()
    } else {
        format!("{crate_name}::{}", components.join("::"))
    }
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, output: &mut Vec<PathBuf>) {
        let mut entries = fs::read_dir(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
            .map(|entry| entry.expect("directory entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                visit(&entry, output);
            } else if entry.extension().is_some_and(|extension| extension == "rs") {
                output.push(entry);
            }
        }
    }
    let mut output = Vec::new();
    visit(root, &mut output);
    output
}

fn type_base_name(kind: &syn::Type) -> Option<String> {
    let syn::Type::Path(kind) = kind else {
        return None;
    };
    kind.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn returns_self(method: &syn::ImplItemFn) -> bool {
    match &method.sig.output {
        syn::ReturnType::Type(_, kind) => type_base_name(kind).as_deref() == Some("Self"),
        syn::ReturnType::Default => false,
    }
}

fn parameters(method: &syn::ImplItemFn) -> Vec<String> {
    method
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => match typed.pat.as_ref() {
                syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

fn has_typed_builder(item: &syn::ItemStruct) -> bool {
    item.attrs.iter().any(|attribute| {
        attribute.path().is_ident("derive")
            && attribute.meta.require_list().is_ok_and(|list| {
                list.tokens
                    .to_string()
                    .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .any(|token| token == "TypedBuilder")
            })
    })
}

fn quoted_setting(compact: &str, setting: &str) -> Option<String> {
    let needle = format!("{setting}=\"");
    let start = compact.find(&needle)? + needle.len();
    let rest = &compact[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn builder_setter_name(field: &syn::Field) -> Result<Option<String>, String> {
    let field_name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .ok_or_else(|| "tuple-struct fields have no generated setter name".to_owned())?;
    let mut prefix = String::new();
    let mut suffix = String::new();
    for attribute in &field.attrs {
        if !attribute.path().is_ident("builder") {
            continue;
        }
        let tokens = attribute
            .meta
            .require_list()
            .map_err(|_| "builder attribute is not a list".to_owned())?
            .tokens
            .to_string();
        let compact = tokens.split_whitespace().collect::<String>();
        if compact.contains("setter(skip)") {
            return Ok(None);
        }
        if compact.contains("strip_bool") {
            return Err(format!("unsupported TypedBuilder grammar '{tokens}'"));
        }
        if let Some(value) = quoted_setting(&compact, "prefix") {
            prefix = value;
        }
        if let Some(value) = quoted_setting(&compact, "suffix") {
            suffix = value;
        }
        // `setter(transform = |...| ...)` and
        // `setter(fn transform<F>(...) ... where ... { ... })` both retain the
        // field name as the generated public setter. We deliberately do not
        // parse the callback body: the option identity is the field name.
    }
    Ok(Some(format!("{prefix}{field_name}{suffix}")))
}

fn insert_option(
    options: &mut BTreeMap<(String, String), PublicOption>,
    symbol: &str,
    option: String,
    entrypoint: String,
) {
    options
        .entry((symbol.to_owned(), option.clone()))
        .or_insert_with(|| PublicOption {
            symbol: symbol.to_owned(),
            option,
            entrypoints: BTreeSet::new(),
        })
        .entrypoints
        .insert(entrypoint);
}

fn scan_files(crate_name: &str, src: &Path, files: impl IntoIterator<Item = PathBuf>) -> Value {
    let mut symbols = BTreeSet::new();
    let mut options = BTreeMap::new();
    for file in files {
        let module = crate_module(crate_name, src, &file);
        let text = fs::read_to_string(&file).expect("read source");
        let syntax = syn::parse_file(&text)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", file.display()));
        for item in &syntax.items {
            match item {
                syn::Item::Struct(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    let qualified = format!("{module}::{}", item.ident);
                    symbols.insert(PublicSymbol {
                        qualified: qualified.clone(),
                        kind: "struct",
                    });
                    let typed_builder = has_typed_builder(item);
                    if let syn::Fields::Named(fields) = &item.fields {
                        for field in &fields.named {
                            let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
                                continue;
                            };
                            if matches!(field.vis, syn::Visibility::Public(_)) {
                                insert_option(
                                    &mut options,
                                    &qualified,
                                    name.clone(),
                                    format!("field:{name}"),
                                );
                            }
                            if typed_builder {
                                match builder_setter_name(field) {
                                    Ok(Some(setter)) => insert_option(
                                        &mut options,
                                        &qualified,
                                        name.clone(),
                                        format!("builder:{setter}"),
                                    ),
                                    Ok(None) => {}
                                    Err(reason) => panic!("{qualified}::{name}: {reason}"),
                                }
                            }
                        }
                    }
                }
                syn::Item::Enum(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    symbols.insert(PublicSymbol {
                        qualified: format!("{module}::{}", item.ident),
                        kind: "enum",
                    });
                }
                syn::Item::Type(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    symbols.insert(PublicSymbol {
                        qualified: format!("{module}::{}", item.ident),
                        kind: "type",
                    });
                }
                _ => {}
            }
        }
        for item in &syntax.items {
            let syn::Item::Impl(item) = item else {
                continue;
            };
            if item.trait_.is_some() {
                continue;
            }
            let Some(owner) = type_base_name(&item.self_ty) else {
                continue;
            };
            let qualified = format!("{module}::{owner}");
            if !symbols.iter().any(|symbol| symbol.qualified == qualified) {
                continue;
            }
            for member in &item.items {
                let syn::ImplItem::Fn(method) = member else {
                    continue;
                };
                if !matches!(method.vis, syn::Visibility::Public(_)) || !returns_self(method) {
                    continue;
                }
                let name = method.sig.ident.to_string();
                if name == "new" || name.starts_with("with_") && method.sig.receiver().is_none() {
                    for parameter in parameters(method) {
                        insert_option(
                            &mut options,
                            &qualified,
                            parameter.clone(),
                            format!("constructor:{}:{parameter}", method.sig.ident),
                        );
                    }
                } else {
                    insert_option(
                        &mut options,
                        &qualified,
                        name.clone(),
                        format!("method:{name}"),
                    );
                }
            }
        }
    }
    json!({
        "schema_version": 3,
        "crate": crate_name,
        "symbols": symbols.into_iter().map(|symbol| json!({
            "qualified": symbol.qualified,
            "kind": symbol.kind,
        })).collect::<Vec<_>>(),
        "options": options.into_values().map(|option| json!({
            "symbol": option.symbol,
            "option": option.option,
            "entrypoints": option.entrypoints.into_iter().collect::<Vec<_>>(),
            "status": "implemented"
        })).collect::<Vec<_>>()
    })
}

pub fn crate_snapshot(root: &Path, crate_name: &str, source: &str) -> Value {
    let src = root.join(source);
    scan_files(crate_name, &src, rust_files(&src))
}

pub fn filtered_snapshot(
    root: &Path,
    crate_name: &str,
    source: &str,
    include: impl Fn(&Path) -> bool,
) -> Value {
    let src = root.join(source);
    let files = rust_files(&src)
        .into_iter()
        .filter(|path| include(path))
        .collect::<Vec<_>>();
    scan_files(crate_name, &src, files)
}

fn flatten_use(prefix: String, tree: &syn::UseTree, output: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            let next = if prefix.is_empty() {
                path.ident.to_string()
            } else {
                format!("{prefix}::{}", path.ident)
            };
            flatten_use(next, &path.tree, output);
        }
        syn::UseTree::Name(name) => {
            output.insert(if prefix.is_empty() {
                name.ident.to_string()
            } else {
                format!("{prefix}::{}", name.ident)
            });
        }
        syn::UseTree::Rename(rename) => {
            output.insert(format!("{}::{} as {}", prefix, rename.ident, rename.rename));
        }
        syn::UseTree::Glob(_) => {
            output.insert(format!("{prefix}::*"));
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix.clone(), item, output);
            }
        }
    }
}

pub fn root_surface_snapshot(root: &Path, crates: &[(&str, &str)]) -> Value {
    let mut result = serde_json::Map::new();
    for (crate_name, lib) in crates {
        let text = fs::read_to_string(root.join(lib)).expect("read crate root");
        let syntax = syn::parse_file(&text).expect("parse crate root");
        let mut exports = BTreeSet::new();
        for item in syntax.items {
            match item {
                syn::Item::Mod(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    exports.insert(format!("mod:{}", item.ident));
                }
                syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    let mut uses = BTreeSet::new();
                    flatten_use(String::new(), &item.tree, &mut uses);
                    exports.extend(uses.into_iter().map(|value| format!("use:{value}")));
                }
                syn::Item::Type(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    exports.insert(format!("type:{}", item.ident));
                }
                syn::Item::Struct(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    exports.insert(format!("struct:{}", item.ident));
                }
                syn::Item::Enum(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    exports.insert(format!("enum:{}", item.ident));
                }
                syn::Item::Fn(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                    exports.insert(format!("fn:{}", item.sig.ident));
                }
                _ => {}
            }
        }
        result.insert((*crate_name).to_owned(), json!(exports));
    }
    Value::Object(result)
}

pub fn assert_snapshot(name: &str, actual: &Value, expected_text: &str, root: &Path) {
    if std::env::var_os("UPDATE_W7_SNAPSHOTS").is_some() {
        let path = root.join("specs").join(name);
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(actual).unwrap()),
        )
        .unwrap_or_else(|error| panic!("cannot update {}: {error}", path.display()));
        return;
    }
    let expected: Value = serde_json::from_str(expected_text).expect("valid W7 snapshot JSON");
    assert_eq!(
        &expected, actual,
        "W7 snapshot {name} is stale; inspect the API change, update execution/tests, then intentionally regenerate with UPDATE_W7_SNAPSHOTS=1"
    );
}
