# Plan 5 - Remove unsafe ThemeData construction and duplicate initialization

## Goal

Eliminate the `MaybeUninit`/raw-field-write construction of `ThemeData` and remove the duplicated safe/unsafe constructor definitions while preserving low native-stack usage.

The final design must make adding a theme field safe by construction. Forgetting to initialize a new field must be a compile error, never potential undefined behavior.

## Current problem

`incular-material/src/foundation/theme_data.rs` currently has:

- a very large flat `ThemeData`;
- a normal safe constructor initializing all fields;
- a separate shared constructor using `Rc::<ThemeData>::new_uninit()`, raw pointer field writes, and `assume_init()`;
- duplicated default initialization logic across both paths.

The unsafe code is locally documented, but maintenance risk is high: adding/reordering/changing fields can make the unsafe constructor incomplete or stale.

## Target architecture

Make `ThemeData` cheap to clone and safe to construct by grouping coherent data rather than manually placing one giant flat value.

Preferred design:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeData {
    inner: Rc<ThemeDataInner>,
}

struct ThemeDataInner {
    core: CoreThemeData,
    colors: LegacyThemeColors,
    components: ComponentThemes,
    extensions: ThemeExtensions,
}
```

The exact groups should follow Material concepts and mutation frequency, not arbitrary field-count chunks.

Possible groups:

- core platform/material policy;
- color scheme + legacy derived colors;
- typography/text/icon themes;
- component theme collection;
- extensions/adaptations.

If public field compatibility is required before 1.0, decide explicitly whether to make a breaking cleanup now. Do not retain an unsafe representation merely to preserve pre-release field access.

## Access model

Prefer read-only accessors for grouped/shared internals. `ThemeData` should retain value semantics from a user's perspective.

For `copy_with`, create a modified inner value safely. Options:

- clone the small affected group(s) and construct a new `ThemeDataInner`;
- use `Rc::make_mut` if copy-on-write semantics are desirable and benchmark well.

Do not expose `Rc` ownership details as part of the public API unless needed.

## Single canonical initialization path

There must be exactly one function responsible for deriving all defaults from a `ColorScheme`.

For example:

```rust
fn defaults_from_color_scheme(color_scheme: ColorScheme) -> ThemeDataInner;
```

Then:

```rust
pub fn from_color_scheme(color_scheme: ColorScheme) -> Self {
    Self { inner: Rc::new(defaults_from_color_scheme(color_scheme)) }
}

pub fn from_color_scheme_shared(color_scheme: ColorScheme) -> Rc<Self> {
    Rc::new(Self::from_color_scheme(color_scheme))
}
```

If stack measurements show this still exceeds the required native-thread budget, solve the size problem structurally by making subgroups heap/shared values. Do not reintroduce field-by-field unsafe placement.

## Stack requirement

Preserve the reason the unsafe path existed: large theme construction must be safe on realistic native UI thread stacks.

Add a deterministic regression test that constructs light/dark/seeded themes on a deliberately small spawned thread stack representative of the minimum supported environment.

The exact stack size should be documented and chosen from supported platform constraints, not guessed solely to make the test pass.

## Migration sequence

1. Measure `size_of::<ThemeData>()` and construction stack behavior.
2. Define coherent grouped structs.
3. Move default derivation to one canonical safe function.
4. Convert read paths to grouped accessors.
5. Convert `copy_with`, `lerp`, `control_theme` and extensions.
6. Remove the raw field-write macro, `new_uninit`, and `assume_init` path.
7. Add small-stack construction tests.
8. Run Miri on `incular-material` tests as an additional regression check.

## Hard invariants

- no unsafe code is needed to construct or clone themes;
- one canonical source defines defaults;
- adding a field to an inner struct requires normal Rust initialization and therefore compiler checking;
- `ThemeData` remains immutable-by-convention/value-semantic to callers;
- `copy_with` does not mutate another clone's observable state;
- Material default values remain identical unless a separate correctness fix is documented;
- native-stack regression is explicitly tested.

## Tests

- light/dark/seeded defaults snapshot or field-by-field compatibility tests;
- `copy_with` preserves unpatched values;
- cloned themes are independent under copy-on-write mutation paths;
- `lerp` behavior remains equivalent;
- `control_theme` bridge remains equivalent;
- extension/adaptation maps survive clones/patches;
- small-thread-stack constructors for light/dark/from-seed;
- size/memory benchmark where useful.

## Acceptance criteria

- `theme_data.rs` contains no `MaybeUninit`, `addr_of_mut!`, field-write unsafe macro or `assume_init`;
- safe/shared constructors use one canonical initialization path;
- small-stack test passes on supported hosts;
- no Material default regression;
- public API changes are intentionally documented before open-source release.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-material
cargo test --workspace --all-features
cargo miri test -p incular-material
```

## Completion report

Report `ThemeData` size before/after, stack-test configuration, removed unsafe sites, public API changes and default-compatibility test results.

### Completed results

- `ThemeData` size: 190,512 bytes before, 8 bytes after on x86_64.
- Construction is fully safe: `incular-material/src` has no `unsafe`, `MaybeUninit`, `new_uninit`, `addr_of_mut!`, or `assume_init` theme path remaining.
- `from_color_scheme` is the single canonical initialization path; shared constructors wrap the same value path.
- The retained value is one `Rc<ThemeDataInner>`; the inner record owns independently shared semantic groups so copy-on-write patches clone only affected groups.
- Group construction is split into helper frames to bound debug/native stack pressure by the largest group rather than the former monolithic 190 KiB descriptor.
- A 256 KiB spawned-thread regression constructs light, dark, seeded, and shared themes successfully.
- Public direct-field access is intentionally replaced pre-1.0 by grouped read accessors (`core()`, `colors()`, `buttons()`, `navigation()`, `selection_controls()`, and related groups).
- `copy_with` copy-on-write independence, shared/value default equivalence, and pointer-sized representation are covered by `theme_data_safety.rs`.
- Full workspace all-feature tests pass after migrating repository call sites.
- Miri was attempted but is unavailable on the installed `stable-x86_64-pc-windows-msvc` toolchain because the `miri` component is not provided there.
