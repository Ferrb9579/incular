# Roadmap

The first release targets experimental desktop applications. The framework's
ownership boundaries are defined in the architecture contract, not Flutter parity.

## Before publication

- Complete the license decision and release checklist in [releasing](docs/releasing.md).
- Verify build checks and native scenarios for every advertised platform.
- Verify archive consumers and the docs.rs build environment; make limitations visible.

## After the initial desktop release

- Improve platform-specific native, accessibility and screen-reader coverage.
- Expand tutorials and examples based on early adopter feedback.
- Track allocation, GPU residency and frame-time regressions on documented hardware.
- Add long-running input/protocol fuzzing and portable Miri coverage where supported.
- Automate API compatibility comparisons once a published baseline exists.
- Complete Android/iOS host lifecycle, input and rendering integration.

No web backend or 1.0 API stability date is promised. Defects in documented
behavior and security fixes take priority over additional widget vocabulary.
