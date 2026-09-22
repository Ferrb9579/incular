# Release preparation status

Updated 2026-09-22. This records implementation and evidence; it is not a claim
that a crates.io version has been published or that every native platform passed.
The initial review is preserved in `system-design/RELEASE_READINESS.md`.

## Implemented

- Rust 1.89 MSRV, matching the locked Linux notification dependency.
- Build-only CI on Windows, Linux and macOS, stable and MSRV. No CI test jobs.
  Pinned Actions, read-only workflow permissions, no persisted checkout credentials,
  and dependency-update configuration.
- Correct virtual-workspace documentation; workspace tests stay in
  `crates/incular/tests/` with no migration/reorganization.
- Metadata and docs.rs target/feature configuration for all 28 framework crates.
- Existing Apache license text in each archive, explicit source inclusion,
  intentional exclusion of repository-only tests/galleries, and a self-contained
  packaged hello application. No license change has been made.
- User-oriented README/rustdoc, application/platform/testing/DevTools guides,
  changelog, roadmap, contribution templates and release/security policies.
- DevTools private exclusive temporary files, bounded/cancellable HTTP upgrades,
  browser-origin rejection and shutdown notification race hardening; four new
  security integration regressions under the crate's `tests/` directory.
- Metadata/archive/link/registry/provenance helpers and a fresh consumer staging
  registry check. The helpers never publish.
- Narrow proposed dependency-license exceptions documented and verified using
  an isolated candidate configuration; actual `deny.toml` is unchanged.

## Evidence

Local logs and generated archives are under ignored `target/` and can be
regenerated using the release runbook. The working tree has not been committed
or uploaded by this task.

| Check | Result |
| --- | --- |
| Initial full all-feature local suite | 2,343 passed across 319 completed groups, zero failures/ignored; live desktop executables may skip by their own guards. |
| New discovery and transport regressions | Four passed, including idle-peer timeout, partial-upgrade shutdown, origin rejection and record ownership/privacy. |
| Rust 1.89 Windows all-feature workspace check | Passed. |
| All-target/all-feature Clippy, warnings denied | Passed after the transport fix. |
| Unused dependency scan | Passed; cargo-machete found none. |
| History secret scan | Gitleaks 8.30.1 scanned 348 commits / about 22.49 MB, no leaks detected. |
| Working-tree source scan, including new files | About 10.57 MB, no leaks detected. No scanner proves absence of all secrets. |
| Registry inventory | All 28 names returned not found at check time; availability is not reserved. |
| Candidate license exceptions | License check passed with three version-specific exceptions; not applied to repository policy. |
| Local Markdown links | Passed when checked; rerun after documentation edits. |
| Final full suite, isolated features, latest archive verification, clean consumer and Linux build/docs | In progress; replace this row with final results before closing preparation. |

## Decisions and account/platform steps still required

1. Choose the project license and dependency exceptions in [licensing.md](licensing.md).
   Recommended: retain Apache-2.0 and accept the three narrow dependency exceptions.
   Actual dependency license policy remains failing until the choice is applied.
2. Enable private vulnerability reporting and confirm a private fallback contact.
   GitHub's API reported reporting disabled; the available browser was signed out,
   so repository settings could not be changed. SECURITY.md states this explicitly.
3. Configure required build checks, branch/tag protection and release ownership in
   GitHub/crates.io with maintainer access. No credentials/owners were invented.
4. Run native macOS and Linux/X11/Wayland/screen-reader scenarios on those systems;
   a container/compiler cannot establish their UI behavior. Keep support claims
   limited until those results exist.
5. After licensing, publish from a clean reviewed commit, verify actual docs.rs
   builds and a real crates.io consumer, and tag the released commit. Those checks
   cannot happen before the first approved upload.

The ttf-parser maintenance advisory stays visible with an assigned maintainer
responsibility and review policy in [dependency risks](dependency-risks.md).
No blanket advisory suppression or speculative text-engine replacement was made.
