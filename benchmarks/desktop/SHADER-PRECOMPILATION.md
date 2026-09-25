# Build-time shader parsing

The linker map identified approximately 1.17 MB of Naga machine code, 1.04 MB of WGPU core and 0.85 MB of WGPU HAL. These are approximate adjacent-symbol sizes after ThinLTO, not exact removable bytes; Naga includes required validation and multiple native code generators.

Incular's 12 rendering shaders and AMD presentation bootstrap are now ordinary WGSL files under `crates/incular-wgpu/src/shaders`. The build script parses and validates them and serializes complete Naga modules. The renderer decodes modules instead of parsing source, then WGPU performs normal device validation and backend compilation. This preserves rectangles, text, images, paths, clipping, blending, blur and other existing effects. No new effects are claimed.

WGPU 30 also parses internal validation shaders at runtime, forcing the WGSL frontend on regardless of the application's input. Vendored WGPU/WGPU-core patches move those shaders through the same build-time parsing approach. Indirect draw and dispatch validation remain active, including device-specific workgroup bounds. Timestamp normalization remains available. The public WGSL feature can still enable arbitrary runtime source shaders for consumers that need it.

See [the dependency patch and maintenance requirements](../../vendor/wgpu/INCULAR-PATCH.md). This approach adds a dependency-maintenance obligation. It does not disable shader validation, remove native backends, strip application controls, reduce supported image formats or remove localization/accessibility.

Validation uses source-versus-embedded-module equivalence for all 13 renderer shaders; DX12, Vulkan and OpenGL shader module validation; indirect GPU dispatch with valid and oversized workgroup counts; timestamp readback when supported; native effect recovery, image tint, two-window resources and resize; and five pixel-identical application captures. No full workspace test suite is rerun for this change. Cross-platform execution on macOS/Linux is not covered by the Windows hardware runs.

[Latest bundle manifest](results/bundle-dist/bundle-manifest.json), [pixel identity](results/bundle-dist/pixel-identity.json), [code attribution](results/bundle-dist/code-attribution.json).

The Vulkan test on this AMD host returns zero timestamp values, including an unpatched upstream WGPU-core 30.0.1 comparison. Zero is recorded as a timing limitation, not a successful nonzero timing measurement. DX12 returns nonzero timestamps. Shader module validation and indirect-dispatch output checks pass independently.
