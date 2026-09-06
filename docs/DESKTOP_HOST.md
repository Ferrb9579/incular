# Desktop host ownership

`run_window(runtime, on_action)` adopts the mounted runtime through
`Application::from_runtime` and delegates to `run_application_with_services`.
There is one Winit `ApplicationHandler`, `DesktopHost`. Native OS facades inject
services into that host; neither the standalone API nor an OS facade owns an
alternate frame loop. Runtime adoption preserves tree identity, reactive builders,
focus, the task scheduler and work queued before the host starts.

## Owners and boundaries

- `window_host.rs` contains each window's renderer, metrics, input state,
  environment, accessibility projection/adapter and owned native window. The
  window field drops last, after every raw-handle consumer. WGPU also retains its
  surface target independently through an owned `Arc<Window>`.
- `input.rs` normalizes mouse chords, device identity, touch/stylus samples,
  keyboard, IME and trackpad input. Top-level and transient windows use the same
  state machine. Focus loss emits cancellation before exit; each window owns
  its own button state while device identities are shared by the host.
- `host_environment.rs` publishes environment observations using only the native
  window records and retained application delivery. It does not own service
  queues or frame scheduling.
- `presentation.rs` submits one window's display list, reports presentation/GPU
  metrics and completes captures. It borrows only that window, the application
  and, when enabled, its diagnostic recorder.
- `native_requests.rs` coordinates shortcut and application-shell completions.
  Application-shell operations consume `DesktopApplicationShellServices`, which
  is independent of window/input/environment methods on `DesktopPlatformServices`.
- `transients.rs` receives a bounded `TransientContext`, rather than the complete
  host. Popup placement, parent-relative coordinate translation, surface fallback
  and teardown stay explicit. Input normalization is shared; a popup's local
  positions are translated into its retained owner's coordinates afterward.

The host orchestrates native creation, cross-window routing, frame scheduling,
menus and service completion. Extracted modules do not import the parent module
with a wildcard. No native resources or Winit types are added to runtime/widgets.

## Backend migration

Winit conversion, IME execution and detached handle access live under
`incular_desktop::winit_adapter`. `incular-platform` keeps portable metrics,
events, native-family diagnostics, capabilities, IDs, commands and typed errors.
It no longer depends on Winit or raw-window-handle. See
[API migrations](API_MIGRATIONS.md#stage-e-desktop-migration) for import and service
trait changes. Application-level convenience runner signatures are unchanged.

## Validation

`input_replay.rs` replays interleaved window/popup input and tests independent
focus cancellation and IME conversion. `runtime_adoption.rs` checks retained
identity, pre-adoption dispatch, task cancellation, legacy action delivery, and
editing/accessibility parity between adopted and declarative roots with one or
multiple windows. Moved conversion tests remain under desktop ownership.
The workspace architecture guard checks runtime/widgets' normal and build
closure for Winit and rejects a platform dependency on it.

The live desktop regressions are opt-in with `INCULAR_DESKTOP_LIVE_TESTS=1`:
`in_place_resize`, `window_control` and `pointer_input_native`. Test results must
name the host platform; headless replay does not establish native behavior on
Linux or macOS. Platform capability limitations recorded by previous stages are
unchanged by this consolidation.
