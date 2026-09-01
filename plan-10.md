# Plan 10 - System Environment, Preferences, Lifecycle, and Occlusion

## Goal

Populate Incular's already-rich `RuntimeEnvironment` from the real desktop OS and complete application/window lifecycle signals.

Current desktop setup mostly supplies viewport/scale and hard-coded input capabilities while leaving theme, locales, reduced motion, and related preferences at defaults.

## Architecture

### Environment provider

Create one per-window/application environment provider that produces normalized snapshots and deltas for:

- brightness/system theme;
- ordered locales;
- text scale/accessibility scaling where meaningful;
- reduced motion;
- input capabilities;
- window focus;
- safe/view insets when desktop concepts apply;
- high-contrast or other accessibility preferences only if modeled explicitly rather than overloading brightness;
- platform lifecycle/activity;
- occlusion where available.

Do not query OS settings from widgets or Material components.

### Change propagation

Only invalidate dependencies affected by changed environment fields. Do not rebuild the entire application blindly for every native setting event.

`MaterialApp::ThemeMode::System` should work because `RuntimeEnvironment.brightness` changes, not through Material-specific platform code.

### Lifecycle

Complete normalization for:

- active/inactive;
- suspended/resumed where OS exposes it;
- stopping/termination intent;
- per-window focus;
- occlusion/minimized visibility where available.

Do not invent lifecycle events an OS cannot provide.

## Crate ownership

- `incular-config`: stable environment values.
- `incular-platform`: provider/event contracts.
- `incular-runtime`: environment dependency invalidation/lifecycle coordination.
- `incular-desktop`: Winit theme/focus/occlusion common paths.
- OS crates: locale/accessibility/reduced-motion and native lifecycle sources.

## Hard invariants

1. Environment snapshots reflect OS state, not framework guesses.
2. System-theme change can update Material theme without recreating windows.
3. Locale preference order is preserved and canonicalized with ICU4X.
4. Reduced-motion changes affect future/current animations according to documented policy.
5. Unsupported settings remain absent/default by explicit policy, not stale cached values.
6. Per-window focus and application lifecycle remain distinct.

## Tests

- Light/dark event updates dependent subtree.
- Locale change resolves ICU4X-supported locale and text direction.
- Reduced motion disables/reduces control animation according to existing tokens.
- Focus transitions do not erase logical text-field focus incorrectly.
- Occlusion can suppress unnecessary frame work where safe without losing invalidation.
- Memory provider allows deterministic environment changes.

## Acceptance criteria

`ThemeMode::System`, locale resolution, reduced motion, and lifecycle behavior work from real native state with no application-side polling.
