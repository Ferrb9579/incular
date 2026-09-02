#[cfg(target_os = "windows")]
use incular_core::{Color, Offset, Size};
#[cfg(target_os = "windows")]
use incular_platform::{CapabilitySupport, WindowOptions};
#[cfg(target_os = "windows")]
use incular_runtime::{
    Application, ResolvedTransientPresentation, Signal, Simulation, SimulationError,
};
#[cfg(target_os = "windows")]
use incular_widgets::{GestureDetector, OverlayPortal, TransientPlacement, TransientRole, Widget};
#[cfg(target_os = "windows")]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    // SAFETY: both pointers are valid for the duration of this synchronous
    // lookup; a null class pointer asks Win32 to match only by title.
    unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
}

#[cfg(target_os = "windows")]
fn owned_windows(
    owner: windows_sys::Win32::Foundation::HWND,
) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        UI::WindowsAndMessaging::{EnumWindows, GW_OWNER, GetWindow, IsWindowVisible},
    };

    struct Context {
        owner: HWND,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn collect(window: HWND, parameter: LPARAM) -> BOOL {
        // SAFETY: `parameter` is the live Context pointer passed to EnumWindows
        // below and the callback is invoked synchronously before it returns.
        let context = unsafe { &mut *(parameter as *mut Context) };
        // SAFETY: EnumWindows supplies a live top-level HWND for the duration
        // of the callback; querying its owner does not retain either handle.
        if unsafe { GetWindow(window, GW_OWNER) } == context.owner
            && unsafe { IsWindowVisible(window) } != 0
        {
            context.windows.push(window);
        }
        1
    }

    let mut context = Context {
        owner,
        windows: Vec::new(),
    };
    // SAFETY: the callback and Context pointer remain valid until this
    // synchronous enumeration completes.
    unsafe {
        EnumWindows(
            Some(collect),
            (&mut context as *mut Context).cast::<core::ffi::c_void>() as LPARAM,
        );
    }
    context.windows
}

#[cfg(target_os = "windows")]
fn window_rect(
    window: windows_sys::Win32::Foundation::HWND,
) -> windows_sys::Win32::Foundation::RECT {
    use windows_sys::Win32::{Foundation::RECT, UI::WindowsAndMessaging::GetWindowRect};

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `window` is discovered from live Win32 enumeration and `rect` is
    // writable for the duration of the synchronous call.
    assert_ne!(unsafe { GetWindowRect(window, &mut rect) }, 0);
    rect
}

#[cfg(target_os = "windows")]
fn is_window(window: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

    // SAFETY: Win32 accepts stale HWND values for this identity query and
    // returns zero once the native window has been destroyed.
    unsafe { IsWindow(window) != 0 }
}

#[cfg(target_os = "windows")]
fn post_primary_click(window: windows_sys::Win32::Foundation::HWND, x: u16, y: u16) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PostMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };

    let lparam = isize::try_from((usize::from(y) << 16) | usize::from(x))
        .expect("Win32 client coordinates fit LPARAM");
    // SAFETY: `window` is a live popup HWND discovered from EnumWindows. These
    // are ordinary queued mouse messages with client-relative coordinates; no
    // pointers or borrowed payloads cross the asynchronous message boundary.
    unsafe {
        assert_ne!(PostMessageW(window, WM_MOUSEMOVE, 0, lparam), 0);
        assert_ne!(PostMessageW(window, WM_LBUTTONDOWN, 1, lparam), 0);
        assert_ne!(PostMessageW(window, WM_LBUTTONUP, 0, lparam), 0);
    }
}

#[cfg(target_os = "windows")]
fn native_transient_surface_smoke() {
    const TITLE: &str = "Incular native transient surface smoke";
    let open = Signal::new(true);
    let observed = open.clone();
    let callback_open = Arc::new(AtomicBool::new(true));
    let callback_open_for_build = Arc::clone(&callback_open);
    let popup_hits = Arc::new(AtomicUsize::new(0));
    let popup_hits_for_build = Arc::clone(&popup_hits);
    let application = Application::new_with_options(
        WindowOptions {
            initial_logical_size: Size::new(100.0, 100.0),
            decorations: false,
            ..WindowOptions::new(TITLE)
        },
        move |_| {
            let click_state = observed.clone();
            let callback_open = Arc::clone(&callback_open_for_build);
            let popup_hits = Arc::clone(&popup_hits_for_build);
            let anchor: Widget =
                GestureDetector::new(Widget::box_(Size::new(100.0, 100.0), Color::BLACK))
                    .on_tap(move || {
                        let next = !click_state.get();
                        callback_open.store(next, Ordering::SeqCst);
                        let _ = click_state.set(next);
                    })
                    .into();
            let popup: Widget =
                GestureDetector::new(Widget::box_(Size::new(80.0, 120.0), Color::WHITE))
                    .on_tap(move || {
                        popup_hits.fetch_add(1, Ordering::SeqCst);
                    })
                    .into();
            OverlayPortal::new(anchor)
                .overlay_child(popup)
                .placement(TransientPlacement::new().alignment_offset(Offset::new(10.0, 0.0)))
                .role(TransientRole::Menu)
                .show(observed.get())
                .into()
        },
    )
    .expect("application");
    let id = application.primary_window();
    let handle = application.window_handle(id).expect("window handle");
    let simulation = application.simulation().window(&handle);
    let worker_handle = handle.clone();
    let worker_callback_open = Arc::clone(&callback_open);
    let worker_popup_hits = Arc::clone(&popup_hits);
    let (sender, receiver) = mpsc::sync_channel(1);

    let worker = std::thread::spawn(move || {
        let result = (|| -> Result<(), SimulationError> {
            frame(&simulation, &worker_handle)?;
            let capabilities = worker_handle.capabilities().transients;
            assert_eq!(capabilities.native_surface, CapabilitySupport::Supported);
            assert_eq!(capabilities.menu, CapabilitySupport::Supported);
            let presentations = worker_handle.transient_presentations();
            assert_eq!(presentations.len(), 1);
            assert_eq!(
                presentations[0].resolved,
                ResolvedTransientPresentation::Native
            );
            assert!(presentations[0].fallback_reason.is_none());

            let parent = find_window(TITLE);
            assert_ne!(parent, 0, "parent HWND must be discoverable");
            let parent_rect = window_rect(parent);
            // Plan 06 deliberately keeps a newly created native host hidden
            // until the runtime has consumed the OS work-area bounds and
            // resolved the same canonical placement as the desktop adapter.
            // Allow bounded frames for that handshake rather than assuming the
            // popup is visible after the first host-creation frame.
            let mut first = owned_windows(parent);
            for _ in 0..8 {
                if first.len() == 1 {
                    break;
                }
                frame(&simulation, &worker_handle)?;
                first = owned_windows(parent);
            }
            assert_eq!(first.len(), 1, "exactly one retained menu host is expected");
            let popup = first[0];
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                };
                // SAFETY: `popup` is a visible HWND returned by EnumWindows.
                let style = unsafe { GetWindowLongPtrW(popup, GWL_EXSTYLE) };
                let expected = isize::try_from(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)
                    .expect("Win32 style flags fit isize");
                assert_eq!(style & expected, expected);
            }
            let popup_rect = window_rect(popup);
            assert!(
                popup_rect.bottom > parent_rect.bottom,
                "native popup must visibly extend below the parent window"
            );
            let parent_width = parent_rect.right - parent_rect.left;
            let parent_height = parent_rect.bottom - parent_rect.top;
            let popup_width = popup_rect.right - popup_rect.left;
            let popup_height = popup_rect.bottom - popup_rect.top;
            let expected_width = (f64::from(parent_width) * 0.8).round() as i32;
            let expected_height = (f64::from(parent_height) * 1.2).round() as i32;
            assert!(
                (popup_width - expected_width).abs() <= 1,
                "popup physical width must match logical width times parent DPI scale"
            );
            assert!(
                (popup_height - expected_height).abs() <= 1,
                "popup physical height must match logical height times parent DPI scale"
            );

            post_primary_click(popup, 20, 20);
            for _ in 0..8 {
                if worker_popup_hits.load(Ordering::SeqCst) > 0 {
                    break;
                }
                frame(&simulation, &worker_handle)?;
            }
            assert_eq!(
                worker_popup_hits.load(Ordering::SeqCst),
                1,
                "native popup-local input must route to the original retained subtree"
            );

            // Stable retained identity must update the existing native host,
            // not churn an owned HWND on every parent frame.
            frame(&simulation, &worker_handle)?;
            assert_eq!(owned_windows(parent), vec![popup]);

            // Close the retained transient through ordinary Incular input. The
            // host must be destroyed while the parent native window survives.
            simulation.click_at(Offset::new(10.0, 10.0))?;
            assert!(
                !worker_callback_open.load(Ordering::SeqCst),
                "simulated anchor tap must run the retained callback"
            );
            let mut closed = false;
            for _ in 0..8 {
                frame(&simulation, &worker_handle)?;
                if owned_windows(parent).is_empty() {
                    closed = true;
                    break;
                }
            }
            assert!(closed, "retained close must destroy the native popup host");
            assert!(worker_handle.transient_presentations().is_empty());
            assert_eq!(worker_handle.id(), id);

            // Reopening uses the same retained portal but creates exactly one
            // fresh native host after the previous one was destroyed.
            simulation.click_at(Offset::new(10.0, 10.0))?;
            assert!(worker_callback_open.load(Ordering::SeqCst));
            let mut reopened = false;
            for _ in 0..8 {
                frame(&simulation, &worker_handle)?;
                if owned_windows(parent).len() == 1 {
                    reopened = true;
                    break;
                }
            }
            assert!(
                reopened,
                "retained reopen must create one native popup host"
            );
            let presentations = worker_handle.transient_presentations();
            assert_eq!(presentations.len(), 1);
            assert_eq!(
                presentations[0].resolved,
                ResolvedTransientPresentation::Native
            );

            // Closing the owning top-level must destroy the transient before
            // or with the parent; an owned popup may never outlive its owner.
            let final_popup = owned_windows(parent)[0];
            worker_handle.close().expect("enqueue parent close");
            let mut popup_destroyed = false;
            for _ in 0..200 {
                let parent_alive = is_window(parent);
                let popup_alive = is_window(final_popup);
                assert!(
                    parent_alive || !popup_alive,
                    "transient HWND must never outlive its owning parent HWND"
                );
                if !popup_alive {
                    popup_destroyed = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                popup_destroyed,
                "parent close must tear down every owned transient HWND"
            );
            Ok(())
        })();
        let _ = worker_handle.close();
        sender.send(result).expect("send native transient result");
    });

    let run_result = incular_windows::run_application(application);
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("native transient smoke timed out")
        .expect("native transient smoke failed");
    worker.join().expect("native transient worker panicked");
    run_result.expect("desktop event loop failed");
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("INCULAR_DESKTOP_LIVE_TESTS").is_none() {
            eprintln!(
                "transient_surface: skipped live native regression; set INCULAR_DESKTOP_LIVE_TESTS=1 to run"
            );
            return;
        }
        native_transient_surface_smoke();
    }
}
