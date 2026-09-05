# Incular

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/Ferrb9579/incular#license)

Incular is an experimental Rust GUI framework inspired by Flutter and built
around `wgpu`. It is currently a research-quality project: APIs and behavior
may change before the first stable release.

## Quick start

```toml
[dependencies]
incular = "0.1"
```

The default feature set includes the desktop runner, Controls, and Material
components. A minimal example is available at
`examples/counter/main.rs`:

```text
cargo run -p incular --example counter
```

## Workspace architecture

The repository is a layered Cargo workspace. The public `incular` facade sits
above focused crates for core types, layout, rendering, widgets, controls,
Material components, runtime scheduling, text, scrolling, accessibility,
platform integration, and the `wgpu` backend. Each crate README documents its
ownership boundary and supported surface.

The [architecture contract](docs/ARCHITECTURE.md),
[accepted decisions](docs/ARCHITECTURE_DECISIONS.md), and
[API migration inventory](docs/API_MIGRATIONS.md) define the current direction.
Flutter manifests are compatibility inventories; Incular's Rust ownership and
behavior contracts determine implementation boundaries.

Run the full validation suite before submitting a change:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test-constrained
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

`cargo test-constrained` caps compilation at four jobs and runs the stock test
harness with one test thread. Use it for full-workspace test runs; focused
crate or test-target commands can continue to use `cargo test -p ...`.

Windows PowerShell uses `$env:RUSTDOCFLAGS='-D warnings'` for the final command.

## Project status

The Linux desktop path is the primary native verification target. Windows and
macOS share the platform runner contract, while Android and iOS integration
are still being developed. The platform table below summarizes the current
implementation coverage and known limits.

| Capability | Linux | Windows | macOS | Android | iOS |
| --- | --- | --- | --- | --- | --- |
| Shared winit/wgpu runner source | Implemented | Source parity | Source parity | Planned | Planned |
| Native runtime verification | Not run here | Verified | Not run here | Planned | Planned |
| Accessibility OS adapter | Planned | Planned | Planned | Planned | Planned |

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Bug
reports and focused implementation contributions are welcome, especially when
they include a regression test or an example simulation.

## License

Incular is licensed under the [Apache License 2.0](LICENSE-APACHE).
