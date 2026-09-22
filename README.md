# Incular

Incular is an experimental native Rust GUI framework with declarative widgets,
reactive state, retained layout/rendering, optional Controls and Material layers,
and a shared Winit/WGPU desktop host. It is intended for early adopters willing
to work with an evolving API. It is not a Flutter binding and does not target web.

## First application

Requires Rust **1.89 or newer**, a desktop window system, and a compatible GPU
driver. See [platform setup](docs/platforms.md) for native prerequisites and limits.
The initial version is being prepared for publication; until it is published,
use the repository checkout instructions below.

After `0.1.0` is published, add:

```toml
[dependencies]
incular = "0.1"
```

Put this in `src/main.rs`, then run `cargo run`:

```rust,no_run
use incular::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Application::new(|_cx| {
        Container::builder()
            .padding(EdgeInsets::all(24.0))
            .alignment(Alignment::CENTER)
            .child(Text::new("Hello, Incular!"))
            .build()
            .into()
    })?;
    incular::run(app)?;
    Ok(())
}
```

The same self-contained [hello example](crates/incular/examples/hello.rs) is
included in the facade crate's archive.

## Try the checkout

```text
git clone https://github.com/Ferrb9579/incular.git
cd incular
cargo run -p incular --example hello
cargo run -p incular --example counter
```

The larger [example galleries](examples/README.md) use repository-local simulation
helpers and require a checkout. They are not part of the registry archive.

## Features

| Feature | Purpose | Default |
| --- | --- | --- |
| `desktop` | Native runner and WGPU backend | Yes |
| `controls` | Incular controls and visual slots | Yes |
| `material` | Material presentation; enables `controls` | Yes |
| `devtools` | Diagnostics transport and runtime instrumentation | No |

Use `default-features = false` to select features explicitly. This selects the
neutral API surface; it does not promise `no_std` or a dependency-free build.
Import neutral types from `incular::prelude`, Controls from
`incular::controls_prelude`, and Material from `incular::material_prelude`.

## Support and learning

Windows, Linux, and macOS share the desktop host. Native runtime evidence varies
by platform; [the support matrix](docs/platforms.md) distinguishes implemented
adapters from verified scenarios. Android/iOS crates provide semantic adapters
for a host, not complete mobile application runners. Desktop accessibility uses
AccessKit; platform/screen-reader behavior needs native validation.

- [Application guide](docs/guide.md)
- [DevTools and diagnostics privacy](docs/devtools.md)
- [Local testing](docs/testing.md)
- [API documentation](https://docs.rs/incular) (available after publication)
- [Roadmap](ROADMAP.md), [changelog](CHANGELOG.md), and [release procedure](docs/releasing.md)

## Workspace and contribution

The root is a **virtual workspace**, not a package. Framework crates live under
`crates/`, the DevTools application under `tools/`, and workspace integration
tests under `crates/incular/tests/`. Crate-specific tests remain in each crate's
`tests/` directory. The [architecture contract](system-design/ARCHITECTURE.md)
defines ownership and application/backend/bridge API classes.

Read [CONTRIBUTING.md](CONTRIBUTING.md), [the code of conduct](CODE_OF_CONDUCT.md),
and [SECURITY.md](SECURITY.md). GitHub Actions performs build checks; tests and
native GUI scenarios are run locally before release.

## License

The existing project license is [Apache-2.0](LICENSE). Dependency licenses are
separate; [the pending licensing decision](docs/licensing.md) records the options
to resolve before publishing. No relicensing is implied by this preparation work.
