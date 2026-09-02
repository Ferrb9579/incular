#[cfg(target_os = "windows")]
use incular_core::Size;
#[cfg(target_os = "windows")]
use incular_platform::WindowOptions;
#[cfg(target_os = "windows")]
use incular_runtime::{Application, Signal, Simulation, SimulationError};
#[cfg(target_os = "windows")]
use incular_widgets::{
    PlatformMenu, PlatformMenuBar, PlatformMenuItem, PlatformMenuShortcut, ShortcutModifiers,
    SizedBox,
};
#[cfg(target_os = "windows")]
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

#[cfg(target_os = "windows")]
fn frame(
    simulation: &Simulation,
    handle: &incular_runtime::WindowHandle,
) -> Result<(), SimulationError> {
    handle.request_redraw().expect("window command bridge");
    simulation.wait_for_frame()
}

#[cfg(target_os = "windows")]
fn find_window(title: &str) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    let title = title
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: the UTF-16 title buffer remains live for the synchronous lookup;
    // a null class pointer asks Win32 to match only the window title.
    unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
}

#[cfg(target_os = "windows")]
fn menu_handles_and_command(
    hwnd: windows_sys::Win32::Foundation::HWND,
) -> (
    windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    u32,
) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetMenu, GetMenuItemCount, GetMenuItemID, GetSubMenu,
    };

    // SAFETY: `hwnd` is a live top-level window discovered by FindWindowW.
    let root = unsafe { GetMenu(hwnd) };
    assert_ne!(root, 0, "PlatformMenuBar must attach a real HMENU");
    // SAFETY: root is the live menu attached to hwnd.
    assert_eq!(unsafe { GetMenuItemCount(root) }, 1);
    // SAFETY: root has one top-level menu item by the assertion above.
    let submenu = unsafe { GetSubMenu(root, 0) };
    assert_ne!(submenu, 0, "top-level File item must own a submenu HMENU");
    // SAFETY: submenu contains the retained leaf command at position zero.
    let command = unsafe { GetMenuItemID(submenu, 0) };
    assert_ne!(command, u32::MAX, "leaf item must have a native command ID");
    (root, submenu, command)
}

#[cfg(target_os = "windows")]
fn menu_item_label(
    submenu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    command: u32,
) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetMenuStringW, MF_BYCOMMAND};

    let mut buffer = [0_u16; 256];
    // SAFETY: submenu/command identify a live menu item and `buffer` is writable
    // for the full synchronous GetMenuStringW call.
    let length = unsafe {
        GetMenuStringW(
            submenu,
            command,
            buffer.as_mut_ptr(),
            i32::try_from(buffer.len()).expect("menu label buffer fits i32"),
            MF_BYCOMMAND,
        )
    };
    assert!(length > 0, "native menu item label must be readable");
    String::from_utf16_lossy(&buffer[..usize::try_from(length).expect("positive length")])
}

#[cfg(target_os = "windows")]
fn menu_item_enabled(
    submenu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    command: u32,
) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetMenuState, MF_BYCOMMAND, MF_GRAYED};

    // SAFETY: submenu/command identify a live native application-menu item.
    let state = unsafe { GetMenuState(submenu, command, MF_BYCOMMAND) };
    state != u32::MAX && state & MF_GRAYED == 0
}

#[cfg(target_os = "windows")]
fn post_menu_command(hwnd: windows_sys::Win32::Foundation::HWND, command: u32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_COMMAND};

    // SAFETY: the posted payload is a scalar Win32 command ID. No pointer to
    // borrowed Rust memory crosses the asynchronous message queue.
    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_COMMAND, command as usize, 0), 0);
    }
}

#[cfg(target_os = "windows")]
fn post_menu_open_close(
    hwnd: windows_sys::Win32::Foundation::HWND,
    submenu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PostMessageW, WM_INITMENUPOPUP, WM_UNINITMENUPOPUP,
    };

    // SAFETY: all posted payloads are scalar Win32 menu handles. No pointer to
    // borrowed Rust memory crosses the asynchronous message queue.
    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_INITMENUPOPUP, submenu as usize, 0), 0);
        assert_ne!(
            PostMessageW(hwnd, WM_UNINITMENUPOPUP, submenu as usize, 0),
            0
        );
    }
}

#[cfg(target_os = "windows")]
fn native_platform_menu_smoke() {
    const TITLE: &str = "Incular native platform menu smoke";
    let phase = Signal::new(0_u8);
    let observed = phase.clone();
    let selected = Arc::new(AtomicUsize::new(0));
    let opened = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicUsize::new(0));
    let selected_for_build = Arc::clone(&selected);
    let opened_for_build = Arc::clone(&opened);
    let closed_for_build = Arc::clone(&closed);

    let application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(360.0, 180.0),
            ..WindowOptions::new(TITLE)
        },
        move |_| {
            let phase_now = observed.get();
            let advance_on_select = observed.clone();
            let advance_on_close = observed.clone();
            let selected = Arc::clone(&selected_for_build);
            let opened = Arc::clone(&opened_for_build);
            let closed = Arc::clone(&closed_for_build);
            let items = if phase_now < 2 {
                vec![
                    PlatformMenuItem::new(if phase_now == 0 { "Open" } else { "Opened" })
                        .id("open")
                        .shortcut(
                            PlatformMenuShortcut::new("O").modifiers(ShortcutModifiers::CONTROL),
                        )
                        .enabled(phase_now == 0)
                        .on_selected(move || {
                            selected.fetch_add(1, Ordering::SeqCst);
                            let _ = advance_on_select.set(1);
                        }),
                ]
            } else {
                vec![
                    PlatformMenuItem::new("Save")
                        .id("save")
                        .shortcut(
                            PlatformMenuShortcut::new("S").modifiers(ShortcutModifiers::CONTROL),
                        )
                        .on_selected(move || {
                            selected.fetch_add(100, Ordering::SeqCst);
                        }),
                ]
            };
            let menu = PlatformMenu::with_items("File", items)
                .id("file")
                .on_open(move || {
                    opened.fetch_add(1, Ordering::SeqCst);
                })
                .on_close(move || {
                    closed.fetch_add(1, Ordering::SeqCst);
                    if phase_now == 1 {
                        let _ = advance_on_close.set(2);
                    }
                });
            PlatformMenuBar::new([menu], SizedBox::new().width(360.0).height(180.0)).into()
        },
    )
    .expect("application");
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let worker_selected = Arc::clone(&selected);
    let worker_opened = Arc::clone(&opened);
    let worker_closed = Arc::clone(&closed);
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(), SimulationError> {
            frame(&simulation, &worker_handle)?;
            let hwnd = find_window(TITLE);
            assert_ne!(hwnd, 0, "native top-level HWND must be discoverable");
            let (root, submenu, command) = menu_handles_and_command(hwnd);
            assert_eq!(menu_item_label(submenu, command), "Open\tCtrl+O");
            assert!(menu_item_enabled(submenu, command));

            post_menu_command(hwnd, command);
            for _ in 0..12 {
                frame(&simulation, &worker_handle)?;
                if worker_selected.load(Ordering::SeqCst) == 1 {
                    break;
                }
            }
            assert_eq!(worker_selected.load(Ordering::SeqCst), 1);

            // Selection flips a Signal that changes only native attributes.
            // The backend must mutate the existing HMENU tree in place rather
            // than replacing native identity for a non-structural update.
            for _ in 0..8 {
                frame(&simulation, &worker_handle)?;
                let current = menu_handles_and_command(hwnd);
                if menu_item_label(current.1, current.2) == "Opened\tCtrl+O" {
                    break;
                }
            }
            let updated = menu_handles_and_command(hwnd);
            assert_eq!(updated.0, root, "root HMENU identity must be retained");
            assert_eq!(
                updated.1, submenu,
                "submenu HMENU identity must be retained"
            );
            assert_eq!(updated.2, command, "native command ID must remain stable");
            assert_eq!(menu_item_label(updated.1, updated.2), "Opened\tCtrl+O");
            assert!(!menu_item_enabled(updated.1, updated.2));

            // Opening/closing the retained submenu exercises native open/close
            // callbacks. Closing advances to a structurally different command
            // tree, which must retire the old command ID permanently.
            post_menu_open_close(hwnd, updated.1);
            let mut replaced = None;
            for _ in 0..12 {
                frame(&simulation, &worker_handle)?;
                let current = menu_handles_and_command(hwnd);
                if menu_item_label(current.1, current.2) == "Save\tCtrl+S" {
                    replaced = Some(current);
                    break;
                }
            }
            assert_eq!(worker_opened.load(Ordering::SeqCst), 1);
            assert_eq!(worker_closed.load(Ordering::SeqCst), 1);
            let replaced = replaced.expect("structural native menu replacement");
            assert_ne!(
                replaced.2, command,
                "removed native command IDs must never be reused"
            );

            // A delayed WM_COMMAND from the removed tree must resolve to no
            // current retained item and therefore cannot hit Save's callback.
            post_menu_command(hwnd, command);
            for _ in 0..3 {
                frame(&simulation, &worker_handle)?;
            }
            assert_eq!(worker_selected.load(Ordering::SeqCst), 1);
            Ok(())
        })();
        let _ = worker_handle.close();
        sender.send(result).expect("send platform-menu result");
    });

    let run_result = incular_windows::run_application(application);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("native platform-menu smoke timed out")
        .expect("native platform-menu smoke failed");
    worker.join().expect("platform-menu worker panicked");
    run_result.expect("desktop event loop failed");
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
            eprintln!(
                "platform_menu: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
            );
            return;
        }
        native_platform_menu_smoke();
    }
}
