# Platform setup and support

Incular requires Rust 1.89+, Cargo, a native linker, and a GPU driver compatible
with WGPU. Rust 2024 is used throughout the workspace. CI builds stable and MSRV
configurations on Windows, Linux, and macOS; a build is not a native QA result.

| Platform | Implementation | Runtime evidence and limits |
| --- | --- | --- |
| Windows | Shared Winit/WGPU host, Win32 services, AccessKit | Architecture inventory records Stage A native lifecycle validation. Run the local live suite for each release. |
| Linux | Shared host, X11/Wayland, portals, AccessKit | Native validation is required on both display protocols. Wayland global positioning/native transient hosting are explicitly unsupported with the current Winit integration. |
| macOS | Shared host, Cocoa services, AccessKit | Native validation is required on supported macOS/GPU configurations. |
| Android | Semantic adapter | Activity, input, surface and system UI require host integration. Not a complete app runner. |
| iOS | Semantic adapter | Application, input, surface and system UI require host integration. Not a complete app runner. |

There is no web target. Accessibility projections exist, but this is not a claim
of complete screen-reader or accessibility-standard conformance.

## Windows

Install Rust's MSVC toolchain and Visual Studio Build Tools with the Desktop
development with C++ workload and Windows SDK. Use an interactive desktop for
GUI scenarios. Software/headless build environments cannot validate presentation.

## Linux

Install a C/C++ build toolchain, pkg-config, and native development packages.
On Ubuntu 24.04 the build job uses:

```sh
sudo apt-get install build-essential pkg-config libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev libxkbcommon-dev libwayland-dev libudev-dev libdbus-1-dev
```

At runtime use an X11 or Wayland desktop with GPU drivers, fonts, a session
D-Bus, and a working xdg-desktop-portal backend for portal file dialogs. Missing
services can produce typed native errors rather than successful placeholders.
Clipboard behavior depends on compositor protocol support. Global window
coordinates are not available under Wayland.

## macOS

Install Xcode command-line tools (`xcode-select --install`) and Rust. Run GUI
applications in a logged-in graphical session. Native permission decisions,
menus, notifications, input, and accessibility need checks on macOS itself.

## Troubleshooting

- Linker errors: verify the platform SDK/native packages before changing Rust code.
- No window/GPU adapter: check the desktop session and driver; a container's
  successful compilation does not imply a presentable surface.
- Missing glyphs: install fonts for the requested language/script.
- Unsupported placement/transient operation: inspect the platform capability
  result; do not infer support from a portable Rust method's presence.
- Rust version failure: use at least the declared MSRV; include Cargo.lock when
  reporting a workspace reproduction.

Record commit, OS, architecture, GPU/driver, WGPU backend, display protocol,
scenario, and outcome for native QA as described in [testing](testing.md).
