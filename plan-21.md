# Plan 21 - Native callback lifetime evidence and truthful documentation

Status: planned. Audit Q07/Q08; specializes plan 15 W9. Depends on plan 20.

Problem/evidence: macOS environment.rs retains observer tokens in Inner while
registered blocks capture Rc<Inner>; cleanup is in Inner::drop. Verify native
retention/thread rules before asserting sound teardown. Several README/header
status claims contradict completed reactive/GPU work.

Root cause to verify: callback ownership may keep its teardown owner alive; safety
comments alone are not a native lifetime proof. Desired ownership: a host-owned
registration lease removes observers before callback state is released, with
explicit main-thread delivery and no retain cycle. Use native API guarantees,
not unsafe Send/Sync or leaked/static lifetime substitutes.

Affected: audited native environment/notification owners and platform tests;
README/support/migration/progress documents. API impact only after caller/native
audit; preserve portable services and typed unsupported outcomes. Remove genuinely
superseded paths, never the deliberately retained painting shim without migration.

Lifecycle/performance: test registration/drop/re-registration, pending callback
after owner close, main-thread delivery and bounded registrations. No permanent
callback capture of its own sole teardown owner.

Acceptance: owner graph and unsafe invariants documented per host, native tests
run on applicable hosts or explicitly left unverified; documentation reconciles
actual code/test evidence. Do not claim Windows builds validate AppKit or Wayland.

Validation: native focused tests on actual hosts, portable lifecycle regressions,
architecture_contract, fmt/check/default+all-feature tests/strict Clippy/rustdoc;
dependency/MSRV checks only when relevant to the change. Record exact executed
coverage, review diff and commit only verified implementation work.
