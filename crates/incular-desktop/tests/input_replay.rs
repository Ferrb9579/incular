use incular_desktop::winit_adapter;
#[path = "../src/input.rs"]
mod input;
#[allow(dead_code)]
#[path = "../src/pointer.rs"]
mod pointer;
use incular_core::{InputEvent, Offset, PRIMARY_POINTER_BUTTON, PointerPhase};
use incular_platform::{PhysicalSize, PlatformEvent, WindowMetrics};
use input::WindowInputState;
use pointer::PointerDeviceRegistry;
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
};

fn replay(
    state: &mut WindowInputState,
    devices: &mut PointerDeviceRegistry,
    event: &WindowEvent,
) -> Vec<PlatformEvent> {
    let translation = state
        .translate(
            event,
            WindowMetrics::new(PhysicalSize::new(300, 150), 1.5),
            devices,
            None,
        )
        .unwrap();
    let _ = translation.kind;
    translation.events.into_iter().flatten().collect()
}
#[test]
fn interleaved_windows_and_popup_local_input_preserve_identical_normalization() {
    let mut isolated = WindowInputState::default();
    let mut main = WindowInputState::default();
    let mut popup = WindowInputState::default();
    let mut isolated_devices = PointerDeviceRegistry::default();
    let mut shared_devices = PointerDeviceRegistry::default();
    let id = DeviceId::dummy();
    let events = [
        WindowEvent::CursorMoved {
            device_id: id,
            position: PhysicalPosition::new(45., 30.),
        },
        WindowEvent::MouseInput {
            device_id: id,
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
        WindowEvent::MouseWheel {
            device_id: id,
            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(0., 2.25)),
            phase: TouchPhase::Moved,
        },
        WindowEvent::MouseInput {
            device_id: id,
            state: ElementState::Released,
            button: MouseButton::Left,
        },
    ];
    for event in &events {
        let expected = replay(&mut isolated, &mut isolated_devices, event);
        assert_eq!(replay(&mut main, &mut shared_devices, event), expected);
        assert_eq!(replay(&mut popup, &mut shared_devices, event), expected);
    }
    assert_eq!(main.cursor, PhysicalPosition::new(45., 30.));
}
#[test]
fn focus_loss_cancels_only_the_owning_window_and_clears_the_button_chord() {
    let mut first = WindowInputState::default();
    let mut second = WindowInputState::default();
    let mut devices = PointerDeviceRegistry::default();
    let down = WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state: ElementState::Pressed,
        button: MouseButton::Left,
    };
    replay(&mut first, &mut devices, &down);
    replay(&mut second, &mut devices, &down);
    let metrics = WindowMetrics::new(PhysicalSize::new(300, 150), 1.5);
    let cancelled = first
        .cancel(metrics)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert!(matches!(
        &cancelled[0],
        PlatformEvent::Input(InputEvent::PointerWithMetadata {
            phase: PointerPhase::Cancel,
            buttons: 0,
            ..
        })
    ));
    assert!(matches!(
        &cancelled[1],
        PlatformEvent::Input(InputEvent::PointerWithMetadata {
            phase: PointerPhase::Exit,
            ..
        })
    ));
    assert_eq!(first.cancel(metrics).into_iter().flatten().count(), 1);
    let moved = WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(30., 15.),
    };
    assert!(
        matches!(&replay(&mut second, &mut devices, &moved)[0], PlatformEvent::Input(InputEvent::PointerWithMetadata { buttons: PRIMARY_POINTER_BUTTON, position, .. }) if *position == Offset::new(20.,10.))
    );
}
#[test]
fn ime_commit_and_end_use_the_same_portable_path() {
    let mut state = WindowInputState::default();
    let mut devices = PointerDeviceRegistry::default();
    let commit = replay(
        &mut state,
        &mut devices,
        &WindowEvent::Ime(winit::event::Ime::Commit("é".into())),
    );
    assert_eq!(
        commit,
        vec![PlatformEvent::Input(InputEvent::Ime(
            incular_core::ImeEvent::Commit("é".into())
        ))]
    );
    assert!(
        replay(
            &mut state,
            &mut devices,
            &WindowEvent::Ime(winit::event::Ime::Enabled)
        )
        .is_empty()
    );
    assert_eq!(
        replay(
            &mut state,
            &mut devices,
            &WindowEvent::Ime(winit::event::Ime::Disabled)
        ),
        vec![PlatformEvent::Input(InputEvent::Ime(
            incular_core::ImeEvent::End
        ))]
    );
}

#[path = "../src/transient_input.rs"]
mod transient_input;
#[test]
fn popup_translation_offsets_positions_once_without_changing_device_or_scroll() {
    let mut state = WindowInputState::default();
    let mut devices = PointerDeviceRegistry::default();
    let event = WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(30., 15.),
    };
    let local = replay(&mut state, &mut devices, &event).pop().unwrap();
    let translated =
        transient_input::offset_transient_platform_event(local, Offset::new(100., 50.));
    assert!(
        matches!(translated, PlatformEvent::Input(InputEvent::PointerWithMetadata { position, device: 1, .. }) if position == Offset::new(120.,60.))
    );
    let scroll = PlatformEvent::Input(InputEvent::Scroll {
        delta: Offset::new(2., 3.),
    });
    assert_eq!(
        transient_input::offset_transient_platform_event(scroll.clone(), Offset::new(100., 50.)),
        scroll
    );
}
