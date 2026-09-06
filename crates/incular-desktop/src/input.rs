//! Stateful native input normalization shared by top-level and popup windows.
use crate::pointer::{MouseButtonState, PointerDeviceRegistry};
use crate::winit_adapter::{
    ime_event, key_event, pointer_event_with_metadata, text_event, touch_event_with_native_sample,
    trackpad_pan_event, trackpad_pinch_event, trackpad_pressure_event, trackpad_rotation_event,
    trackpad_smart_magnify_event, wheel_event,
};
use incular_core::{PointerDeviceKind, PointerPhase};
use incular_platform::{NativePointerSample, PlatformEvent, PointerMetadata, WindowMetrics};
use winit::{dpi::PhysicalPosition, event::WindowEvent, keyboard::ModifiersState};

#[derive(Clone, Copy)]
pub(crate) enum InputKind {
    Mouse,
    Touch,
    Stylus,
    Keyboard,
    Trackpad,
    Other,
}
pub(crate) struct InputTranslation {
    pub(crate) kind: InputKind,
    pub(crate) events: [Option<PlatformEvent>; 2],
}
#[derive(Default)]
pub(crate) struct WindowInputState {
    pub(crate) cursor: PhysicalPosition<f64>,
    pub(crate) modifiers: ModifiersState,
    buttons: MouseButtonState,
}
impl WindowInputState {
    pub(crate) fn translate(
        &mut self,
        event: &WindowEvent,
        metrics: WindowMetrics,
        devices: &mut PointerDeviceRegistry,
        native: Option<NativePointerSample>,
    ) -> Option<InputTranslation> {
        let mut second = None;
        let (kind, first) = match event {
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                self.cursor = *position;
                (
                    InputKind::Mouse,
                    Some(self.mouse(devices.id(*device_id), PointerPhase::Move, None, metrics)),
                )
            }
            WindowEvent::CursorEntered { device_id } | WindowEvent::CursorLeft { device_id } => {
                let phase = if matches!(event, WindowEvent::CursorEntered { .. }) {
                    PointerPhase::Enter
                } else {
                    PointerPhase::Exit
                };
                (
                    InputKind::Mouse,
                    Some(self.mouse(devices.id(*device_id), phase, None, metrics)),
                )
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                let transition = self.buttons.transition(*state, *button);
                let device = devices.id(*device_id);
                (
                    InputKind::Mouse,
                    transition.map(|t| self.mouse(device, t.phase, Some(t.button), metrics)),
                )
            }
            WindowEvent::Touch(touch) => {
                let kind = if native.is_some_and(|sample| {
                    matches!(
                        sample.kind,
                        PointerDeviceKind::Stylus | PointerDeviceKind::InvertedStylus
                    )
                }) {
                    InputKind::Stylus
                } else {
                    InputKind::Touch
                };
                let (device, native) = devices.resolve_native_sample(touch.device_id, native);
                (
                    kind,
                    Some(touch_event_with_native_sample(
                        *touch, device, native, metrics,
                    )),
                )
            }
            WindowEvent::PinchGesture {
                device_id,
                delta,
                phase,
            } => (
                InputKind::Trackpad,
                trackpad_pinch_event(devices.id(*device_id), *delta, *phase),
            ),
            WindowEvent::RotationGesture {
                device_id,
                delta,
                phase,
            } => (
                InputKind::Trackpad,
                trackpad_rotation_event(devices.id(*device_id), *delta, *phase),
            ),
            WindowEvent::PanGesture {
                device_id,
                delta,
                phase,
            } => (
                InputKind::Trackpad,
                trackpad_pan_event(devices.id(*device_id), *delta, *phase, metrics),
            ),
            WindowEvent::DoubleTapGesture { device_id } => (
                InputKind::Trackpad,
                Some(trackpad_smart_magnify_event(devices.id(*device_id))),
            ),
            WindowEvent::TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => (
                InputKind::Trackpad,
                trackpad_pressure_event(devices.id(*device_id), *pressure, *stage),
            ),
            WindowEvent::MouseWheel { delta, .. } => {
                (InputKind::Mouse, Some(wheel_event(*delta, metrics)))
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                (InputKind::Other, None)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                second = text_event(event);
                (InputKind::Keyboard, Some(key_event(event, self.modifiers)))
            }
            WindowEvent::Ime(event) => (InputKind::Keyboard, ime_event(event.clone())),
            _ => return None,
        };
        Some(InputTranslation {
            kind,
            events: [first, second],
        })
    }
    fn mouse(
        &self,
        device: u64,
        phase: PointerPhase,
        button: Option<u32>,
        metrics: WindowMetrics,
    ) -> PlatformEvent {
        pointer_event_with_metadata(
            PointerMetadata {
                pointer: 0,
                device,
                kind: PointerDeviceKind::Mouse,
                buttons: self.buttons.pressed(),
                button,
                phase,
                sample: Default::default(),
            },
            self.cursor,
            metrics,
        )
    }
    pub(crate) fn cancel(&mut self, metrics: WindowMetrics) -> [Option<PlatformEvent>; 2] {
        let pressed = self.buttons.cancel();
        [
            pressed.then(|| self.mouse(0, PointerPhase::Cancel, None, metrics)),
            Some(self.mouse(0, PointerPhase::Exit, None, metrics)),
        ]
    }
}
