# Release runbook

Release scope: experimental `0.1.0`, all 28 framework crates, including explicitly
partial Android/iOS semantic adapters. The DevTools UI remains unpublished.
Keep workspace integration tests in `crates/incular/tests/`. CI runs build
checks only; the release maintainer runs the local test/native checklist.

## Before tagging

1. Resolve [licensing](licensing.md). Keep the root and all crate license texts
   identical; run `python scripts/release.py metadata`. Confirm rights/provenance
   for any externally adapted source/assets; a generated dependency inventory
   is not proof of authorship.
2. Update CHANGELOG.md, platform limitations and version/MSRV policy. Application
   API breaking changes require a new 0.x minor line; patch releases preserve
   compatibility. Bridge APIs still need mutually compatible crate dependencies.
   Update workspace versions and internal requirements together, including
   explicit prerelease versions if a prerelease is chosen.
3. Run [local tests and native scenarios](testing.md), the build matrix, and
   `cargo +1.89.0 check --workspace --all-features --locked`. Record actual OS
   evidence; do not silently translate unavailable platform checks to “passed”.
4. Run `cargo deny check -W unmaintained`, `cargo machete --with-metadata`,
   `python scripts/release.py inventory`, and `python scripts/release.py links`.
   Review [dependency risks](dependency-risks.md); do not suppress unexpected findings.
5. Run a current secret scanner on the working tree and all history intended for
   publication, with redaction enabled. Inspect final package archives too. If a
   credential is found, revoke it before any history cleanup or publication.
6. Verify crate names/owners with `python scripts/release.py registry`. A missing
   name is available only at check time; a matching repository field on an
   existing crate is not proof that the release account owns it.

## Package and documentation rehearsal

Use a release toolchain with workspace packaging support (verified here with
Cargo 1.98). This is separate from the library's Rust 1.89 MSRV.

```text
python scripts/release.py package
python scripts/check-consumer.py
```

For local pre-commit preparation only, `package --allow-dirty` is available.
The final release must use a clean reviewed commit. The helper packages and
build-verifies the archives, audits their contents, checks embedded license
copies and target paths, and records SHA-256 hashes under `target/`.
It never uploads. Cargo's warnings about omitted repository test/gallery targets
are expected; those targets must not remain in normalized manifests.

The consumer helper starts a loopback staging registry serving the exact archives
and forwarding external dependencies to crates.io. It uses source replacement,
not path dependencies or workspace patches. It builds the minimal library and
desktop hello application and checks all features. It needs network access and
stops the staging server on completion.

Run strict rustdoc on native targets. Validate the docs.rs Linux/nightly build
environment too; its sandbox has no build-time network and read-only sources.
Native crate metadata chooses a representative native target; the facade also
requests Windows/macOS pages. Verify those cross-target pages actually expose
the intended APIs. A local Windows rustdoc pass alone does not prove this.
See [docs.rs build guidance](https://docs.rs/about/builds).

## Account and repository setup

- Verify the release account's crates.io email, ownership and recovery method.
- Enable [private vulnerability reporting](https://github.com/Ferrb9579/incular/settings/security_analysis)
  and verify the [report form](https://github.com/Ferrb9579/incular/security/advisories/new).
  It was disabled during preparation; changing local SECURITY.md does not enable it.
- Confirm a private maintainer/conduct fallback contact before community launch.
- Require the build workflow on `master`, restrict force pushes/deletion, and
  protect release tags. Keep Actions permissions minimal and publisher credentials
  unavailable to pull-request jobs. Branch/tag protection is a GitHub setting,
  not something these local workflow files can enforce by themselves.
- After initial publication, configure crates.io Trusted Publishing for the
  exact repository/workflow and protected release environment. Do not add a
  long-lived token to this repository. Verify current bootstrap requirements in
  [crates.io's documentation](https://crates.io/docs/trusted-publishing).

## Publication and recovery

Only publish after all gates and the license choice are complete. Rehearse with
`cargo publish --workspace --exclude incular-devtools-ui --dry-run --locked` from
the final commit. Then use the same package selection for publication.
Regenerate dependency layers with `python scripts/release.py metadata`; modern
Cargo handles the workspace operation, but the upload is not atomic.

Record every successful package/version and verify index availability. After a
timeout, inspect the registry before retrying; skip already published versions.
Do not increment a subset of dependent crates blindly. Uploads are immutable;
yanking does not remove the archive or secrets and does not repair lockfiles.
For a bad release publish a corrected version, consider yanking the affected
version, and explain the impact in the changelog/advisory.

Before announcing, verify every docs.rs build, registry README, license and
repository link; build a new external application against crates.io; tag the
exact released commit and attach curated release notes. Keep known platform
limits visible. Repeat native/security checks for subsequent releases.
