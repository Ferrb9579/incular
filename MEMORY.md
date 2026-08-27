# Incular development memory policy

Every repository command that can compile, link, test, benchmark, run an
example, or inspect a large artifact must start with an 8 GiB virtual-memory
limit. Do not run an unconstrained Cargo command:

```bash
ulimit -v 8388608
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 RUST_TEST_THREADS=1
```

`.cargo/config.toml` carries the serial build/test defaults as a second line of
defense. Keep an eye on both resident memory and disk (`free -h`, `df -h .`,
and `du -sh target`). Avoid broad example-heavy workspace test builds when the
disk is near capacity; check examples separately with
`cargo check -p incular --examples`. If `target/` pushes the filesystem toward
exhaustion, stop the build, inspect the target size, and use `cargo clean`
before retrying. Never use `sudo`; use `pkexec` only when a local OS action
actually requires elevation.
