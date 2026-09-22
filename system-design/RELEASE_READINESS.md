# Open-source and first-release plan

Initial review date: 2026-09-21. Scope: publish Incular's source, release its intended
packages on crates.io, and provide usable documentation on docs.rs.

Recommendation: make the first release an explicitly experimental desktop
release, with support claims tied to evidence. Do not make full mobile support,
Flutter parity, or a 1.0 API guarantee prerequisites. Resolve the release gates
below before publishing packages or broadly announcing the library.

This is a readiness assessment and implementation plan, not a complete security
audit. It is based on targeted source inspection and local Windows checks.
GitHub settings, crate-name availability/ownership, historical secrets, and native
Linux/macOS behavior have not been verified. No packages have been published.

Implementation is tracked in [the release status](../docs/release-status.md).
The maintainer subsequently specified build-only CI and confirmed that workspace
tests intentionally live in `crates/incular/tests/`. Licensing decisions remain
deferred to the maintainer. Findings below describe the initial baseline.

## What already exists

- 28 framework crates, all configured for publication at `0.1.0`; the DevTools
  application is a separate workspace member with `publish = false`.
- Workspace descriptions, Apache-2.0 license declarations, repository/homepage
  metadata, versioned local dependencies, and a checked-in Cargo.lock.
- Architecture and API ownership contracts, a machine-readable dependency policy,
  and substantial integration tests and example simulations.
- CONTRIBUTING.md, SECURITY.md, deny.toml, and bounded build/test configuration.
- Opt-in DevTools with loopback binding, random session authentication, message
  limits, and discovery-record limits; image decoding and cache budgets also exist.

Keep these foundations. The main gap is reproducible release evidence and an
approachable public distribution, rather than a need to reorganize the framework.

## Confirmed findings at the initial review

| Priority | Finding and evidence | Required outcome |
| --- | --- | --- |
| P0 | `cargo deny check -W unmaintained` fails its license policy: `clipboard-win 5.4.1` and `error-code 3.4.0` use BSL-1.0; `ksni 0.3.6` uses Unlicense. Neither is allowed in deny.toml. | Review the actual dependency license obligations; deliberately update policy or change dependencies. Do not silence the check globally. This is a policy failure, not evidence that those licenses are incompatible. |
| P0 | Cargo.toml declares Rust 1.88, while metadata for the locked `notify-rust 4.18.0` declares 1.89.0. `incular-linux` enables it on Linux. | Raise the advertised MSRV consistently or select a compatible dependency; prove the chosen MSRV on supported targets and in a fresh consumer. Windows-only checks miss this mismatch. |
| P0 | No `.github/` directory or checked-in CI workflows were found. | Establish build-only CI for the public support matrix; run tests locally. Remote repository protections remain a separate settings check. |
| P0 | `cargo package --list` for `incular` and `incular-core` contains no license file. The root README links to nonexistent `LICENSE-APACHE`; the actual file is `LICENSE`. | Fix the link and intentionally include license text and applicable third-party notices in every published archive; verify the final archive lists. Cargo accepting an SPDX field alone is not this project's distribution gate. |
| P0 | The facade manifest points example/test targets outside its package (`../../examples/...`). Its package listing omits those examples. Packaged `tests/architecture_contract.rs` embeds `../../../specs/architecture.json`, also absent from the archive. | Decide which examples/tests are repository-only; make packaged targets self-contained and test extracted archives. A library-only packaging build does not prove packaged tests work. |
| P1 | `crates/incular/README.md` is only an architecture/support table, and the facade's crate-level rustdoc introduction is four lines. The root quick start combines a registry dependency with a checkout-only example command. | Provide a complete copyable consumer quick start, useful crate landing pages, and separate checkout instructions. |
| P1 | All 29 package metadata records lack docs.rs configuration, keywords, and categories. | Add useful discovery metadata and select/test docs.rs targets and features for each publishable package. These fields are quality improvements, not all registry requirements. |
| P1 | README and AGENTS.md describe a root integration-test package, but Cargo.toml is a virtual workspace and repository policy tests live under `crates/incular/tests/`. README's accessibility table also says every OS adapter is planned while the architecture inventory describes existing projections. | Reconcile documentation, architecture evidence, and test ownership with the actual layout and validated platform behavior. |
| P1 | SECURITY.md offers private reporting only “when available” and no concrete fallback address. No changelog, code of conduct, release workflow, or release runbook was found. | Establish a working private reporting route, version-support policy, release history, and contributor/maintainer expectations. |
| P1 | The dependency advisory check reports `ttf-parser` as unmaintained, RUSTSEC-2026-0192, through `fontdue` and `ab_glyph` dependencies. | Record an owner and mitigation/migration decision. An unmaintained advisory is not itself proof of an exploitable vulnerability. |

P0 means a first-publication gate. P1 means required for the public launch unless
the affected feature is explicitly deferred. P2 work below can follow the first
experimental release.

## Workstream 1 — Release scope and packaging (P0)

- [ ] Choose `0.1.0` with a clear experimental policy, or a prerelease such as
  `0.1.0-alpha.1`. If using a prerelease, update workspace versions and internal
  dependency requirements together; verify resolution rather than only changing
  `[workspace.package].version`.
- [ ] Inventory names and owners for all 28 candidate crates on crates.io.
  Check spelling and availability before finalizing public docs. Do not publish
  empty placeholders to reserve names.
- [ ] Keep the main facade as the recommended application entry point. Publish
  its complete dependency closure, including optional and target-specific
  dependencies needed by supported configurations. Backend/bridge API classes
  do not mean their dependency packages can remain unpublished.
- [ ] Decide whether standalone Android/iOS semantic adapters are part of this
  release. If included, describe host-wiring requirements; if deferred, exclude
  them consistently from the release set. Keep the DevTools UI unpublished unless
  a separate installation/distribution plan is deliberately added.
- [ ] Add package documentation links, focused keywords/categories, and explicit
  archive-content rules where needed. Ensure license/notice files, shaders,
  generated sources, and compile-time assets are present; omit local diagnostics
  and repository-only material. Inspect actual archives, not just manifests.
- [ ] Keep canonical repository examples under `examples/<example>/` and tests
  under `tests/` directories. Resolve archive isolation using standalone example
  packages, a small self-contained packaged example, or explicit exclusion of
  repository-only targets. Update layout/architecture tests with any approved
  ownership change; do not move test helpers into `src/`.
- [ ] Audit normal/build/optional/target and development dependency edges. Several
  tests compose higher layers, so the runtime DAG below alone is insufficient to
  prove first-publication ordering and archive test resolution.
- [ ] Use a tested release toolchain. Installed Cargo 1.98 exposes workspace
  publication, but release-tool compatibility is separate from library MSRV.
  Rehearse `cargo publish --workspace --dry-run` with the exact release set, or
  equivalent explicit package selection, before choosing the final automation.
- [ ] Build an external sample without workspace patches or path dependencies.
  Before upload use a staged/local registry with the actual archives; after
  upload repeat against crates.io. Exercise default and minimal features.

Acceptance: every selected archive is self-contained, registry dependency
resolution succeeds, publish verification passes, and a new consumer builds.
Avoid `--no-verify` as a release-gate workaround. Cargo normalizes manifests and
replaces local dependency paths when packaging; inspect that result explicitly.
See [Cargo packaging](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
and [publication options](https://doc.rust-lang.org/cargo/commands/cargo-publish.html).

Current normal/build dependency layers, including optional and target edges:

```text
1  assets, core, devtools-protocol
2  animation, config, gestures, semantics
3  accessibility, image, layout, platform, scroll
4  android, ios, rendering
5  text, wgpu
6  widgets
7  controls, navigation
8  material, runtime
9  devtools
10 desktop
11 linux, macos, windows
12 incular
```

Names except `incular` have the `incular-` prefix. Regenerate this graph from
Cargo metadata after changes; it is planning evidence, not a substitute for a
successful full release rehearsal.

## Workstream 2 — CI, final code fixes, and platform evidence (P0/P1)

- [ ] Add CI jobs for Linux, Windows, and macOS on stable Rust, installing the
  platform dependencies needed by the desktop stack. Test the declared MSRV
  separately. Keep the lockfile for repeatability and also check a fresh
  downstream dependency resolution so the workspace lockfile cannot hide problems.
- [ ] Require formatting, workspace builds, feature checks, Clippy and rustdoc in
  build-only CI. Run constrained tests, doctests, architecture policy and
  test-placement checks locally; do not add CI test jobs.
- [ ] Check the facade separately with no defaults, desktop only, controls only,
  material, defaults, and defaults plus DevTools. Select library targets where
  examples require unavailable features, or declare correct `required-features`.
  Workspace feature unification and `--all-features` alone do not prove these modes.
- [ ] Review the public application API against system-design/API_DESIGN.md:
  naming, error types, ownership, callback reentrancy, cancellation, thread
  restrictions, identity lifetimes, and feature-gated exports. Remove accidental
  public internals before publication; `#[doc(hidden)]` is still public Rust API.
- [ ] Make the bridge compatibility policy actionable: semver-compatible releases
  must preserve cross-crate contracts, or internal requirements must enforce the
  versions that actually work together. Add API-diff checks after a baseline exists.
- [ ] Triage production panics/unwraps by reachability and contract. Convert
  environmental/native/input failures to typed errors; document legitimate
  programmer-invariant panics. Do not mechanically replace every unwrap.
- [ ] Review unsafe/FFI blocks for window-handle ownership, main-thread affinity,
  callback lifetime, and teardown ordering. Add focused regression tests for
  confirmed faults; use Miri for suitable portable code, not the native GPU stack.
- [ ] Exercise text/IME and Unicode selection, keyboard/focus, scrolling,
  accessibility, resizing/DPI, clipboard, dialogs, multi-window shutdown,
  surface/device failure, task cancellation, and image/font resource limits.
  Include X11/Wayland and native screen-reader checks in the relevant OS evidence.
- [ ] Run native GPU/window tests explicitly: several desktop test executables
  skip unless `INCULAR_DESKTOP_LIVE_TESTS` is set. A green ordinary test run does
  not establish native runtime support. Record OS, GPU/backend, tested commit,
  passing scenarios, and known limitations.

Acceptance: required CI is green on the supported matrix; claimed runtime
capabilities have native evidence; remaining defects are fixed, excluded from
scope, or clearly documented with an owner. Mobile compile checks do not imply
working mobile applications. No web target is planned.

## Workstream 3 — Documentation and docs.rs (P1)

- [ ] Rewrite the root and facade READMEs for users: what Incular is useful for,
  experimental status, screenshot, supported OS/MSRV, installation prerequisites,
  complete hello-world/counter code, features, examples, and links for help.
  The consumer example must work without `examples/tests/support` or other repo
  files; explain `git clone` before checkout-only `cargo run --example` commands.
- [ ] Add a compact guide under `docs/`: getting started; reactive state and
  lifecycle; layout/widgets; controls/material/theming; async tasks; navigation;
  text/input; assets/rendering; accessibility; native services; testing; DevTools;
  platform setup/troubleshooting. Prioritize a coherent first-app tutorial, then
  link focused examples for advanced topics.
- [ ] Expand rustdoc for application-facing APIs with examples, error/panic
  contracts, feature/target availability, and safety/thread/lifetime constraints
  where relevant. Introduce `missing_docs` incrementally rather than hiding a
  large new warning set; keep broken intra-doc links denied.
- [ ] Audit Markdown links from GitHub, crates.io README rendering, and docs.rs.
  Crate READMEs currently link outside their archives to architecture documents;
  use appropriate repository URLs for repository-only documentation.
- [ ] Add `[package.metadata.docs.rs]` per publishable crate where needed. Select
  representative features deliberately and choose real documentation targets for
  native APIs. Start with a small proven target set; default Linux documentation
  can hide Windows/macOS exports. Use `doc(cfg)` where useful and test any docsrs
  conditional compilation on nightly.
- [ ] Reproduce docs.rs builds from packages in its Linux environment, with no
  network access during builds and no writes to source directories. Test native
  dependency/cross-compilation constraints and inspect the rendered facade,
  preludes, and backend pages, not just the command exit code.

Acceptance: a new user can build the tutorial, determine supported features and
platform limits, and navigate complete API documentation. docs.rs automatically
builds crates.io releases; it is not a separate source upload. Verify the actual
build/page after publication. See [docs.rs builds](https://docs.rs/about/builds)
and [metadata configuration](https://docs.rs/about/metadata).

## Workstream 4 — Security, licensing, and repository cleanup (P0/P1)

- [ ] Scan the working tree, all Git history intended for publication, fixtures,
  screenshots, logs, and package archives for credentials/private data using a
  secret scanner. Revoke exposed credentials first; decide whether history
  remediation is needed before making source public. Do not print secrets in logs.
- [ ] Review provenance and redistribution notices for copied/adapted code,
  fonts, images, shaders, and Flutter/Material reference material. Preserve
  required attribution; add a third-party notices file where applicable.
- [ ] Resolve deny.toml's three license failures and give the unmaintained
  `ttf-parser` paths an explicit disposition. Run dependency/advisory checks in
  CI and periodically; record any narrowly scoped exception with a reason,
  responsible maintainer, and review date. Also check unused dependencies.
- [ ] Threat-model DevTools and native IPC: local/remote access boundaries,
  authentication failures, browser-origin access, handshake timeout, oversized
  messages, client counts, queue budgets, reconnection, and shutdown.
- [ ] Specifically review `crates/incular-devtools/src/discovery.rs:88-94`:
  it uses a predictable temporary name, writes the token-bearing JSON, then
  applies Unix mode 0600. Assess parent-directory access and symlinks; prefer
  secure exclusive creation with restrictive permissions from the start.
  Verify Windows ACL behavior and record cleanup too. This is a source-level
  hardening concern; exploitability has not been demonstrated in this review.
- [ ] Test malformed images/fonts, restoration data, and diagnostic messages
  with bounded resource use. Existing limits are a starting point; add focused
  fuzz/property tests for externally supplied data and retain regressions under
  `tests/` as required by the repository policy.
- [ ] Document diagnostics privacy: logs, widget labels/text, file paths,
  screenshots, discovery credentials, and crash dumps. Verify DevTools remains
  opt-in and make sensitive capture/retention behavior clear to applications.
- [ ] Enable and test GitHub private vulnerability reporting; give SECURITY.md
  an exact reporting link, real fallback contact, supported versions, and a
  realistic response target. Establish an advisory/patch release process.
- [ ] Add CHANGELOG.md, a concise roadmap, issue/PR templates, CODE_OF_CONDUCT.md
  with an actual enforcement contact, and maintainer/release ownership guidance.
  Review obsolete comments/status claims and local machine references; avoid a
  large cosmetic refactor immediately before release.
- [ ] Configure branch/tag protection, required checks, least-privilege Actions
  permissions, pinned third-party actions, dependency updates, and secure release
  credentials. Never expose publishing credentials to untrusted PR jobs.

Acceptance: no unresolved discovered credential leaks or known release-blocking
security defects; dependency policy passes; the private reporting route works;
all distributed material has documented provenance and required notices.

## Workstream 5 — Rehearsal, publication, and maintenance

1. Create a release checklist and assign a maintainer to each unresolved item.
   Choose the supported versions/platforms and freeze feature scope.
2. Complete a release PR covering versions, manifests, package contents,
   changelog, documentation, and known limitations. Run every release gate from
   the exact intended commit and archive the results.
3. Verify crates.io account access, email/ownership, package names, and a recovery
   route. Rehearse without upload using current Cargo's workspace/package
   selection. Confirm optional/target dependencies and development-edge behavior.
4. Publish the approved dependency set using the rehearsed order/tool. A
   multi-crate release is not atomic: record each successful upload, wait for
   registry availability, and make retries skip already-published versions.
   Recheck registry state after timeouts before attempting recovery.
5. Verify every docs.rs build and every registry package, then build/run a clean
   external sample on supported platforms. Tag the exact released commit and
   attach curated release notes and known limitations before announcing.
6. Configure crates.io Trusted Publishing for subsequent releases, binding the
   exact repository/workflow and protected release environment. Verify current
   first-publication setup requirements rather than assuming an unpublished name
   already supports the desired authentication flow. See the official
   [Trusted Publishing announcement](https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07/)
   and [later enhancements](https://blog.rust-lang.org/2026/01/21/crates-io-development-update/).
7. Maintain a patch/yank runbook: versions cannot be overwritten; a yank prevents
   new resolution but does not remove an archive or repair consumers' lockfiles.
   Publish a corrected version and communicate the impact. See
   [Cargo's publication and yank guidance](https://doc.rust-lang.org/cargo/reference/publishing.html).

P2 after launch: broader platform/device coverage, long-running fuzzing, memory
and frame-time regression budgets, API compatibility automation, expanded guide
content, contributor onboarding, and mobile host work. Do not delay a clearly
scoped experimental release for a custom documentation website or complete parity.

## Validation record

Local environment: Windows x86_64 MSVC, Cargo/rustc 1.98.0. These results do not
establish MSRV compatibility, native Linux/macOS behavior, or docs.rs success.

| Check | Review result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed. |
| `cargo check --workspace --all-targets --locked` | Passed. |
| `cargo metadata --locked --format-version 1` | Inspected; identified the Linux dependency MSRV mismatch. |
| `cargo package -p incular-core --list` and `cargo package -p incular --list` | Passed listing; revealed missing license text and facade example/spec omissions. Does not verify archive builds. |
| `cargo deny check -W unmaintained` | Failed license policy for three dependencies; advisories/bans/sources categories passed, with maintenance/duplicate warnings. |
| `cargo deny check advisories -W unmaintained` | Passed with the `ttf-parser` maintenance warning. |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --all-features --no-deps --locked` | Passed on Windows; 29 documentation outputs generated. This is not a docs.rs/Linux rehearsal. |
| `cargo test-constrained --all-features --locked` | Started, then stopped during extended test/example compilation and linking to keep this documentation-only review scoped. No completed test-suite result; must run before release. Local partial log: `target/release-readiness-tests.log`. |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Passed on Windows. |
| MSRV, live native tests, archive build/dry-run, clean registry consumer, docs.rs environment, history secret/provenance audit | Not performed; explicit implementation/release gates above. |

Implement in this order: packaging/MSRV/license fixes; mandatory CI; user docs
and docs.rs rehearsal; focused code/security fixes and native QA; final release
rehearsal and publication. Documentation and policy preparation can progress
alongside CI work, but publication depends on all relevant acceptance criteria.
