//! High-level, in-process interaction and capture for Incular applications.
//!
//! The simulator is deliberately below the native desktop boundary. It sends
//! the same normalized [`InputEvent`] values that a platform adapter sends,
//! after resolving semantic labels to retained layout bounds. No OS cursor,
//! keyboard device, focus, or desktop screenshot API is involved.

use super::{Application, Constraints, InputEvent, WindowHandle, WindowId};
use incular_core::{
    Code, KeyState, KeyboardEvent, KeyboardKey, Location, Modifiers, NamedKey, Offset,
    PointerPhase, Rect,
};
use incular_semantics::SemanticActionKind;
use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const SIMULATION_REPLY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_SIMULATION_REQUESTS_PER_TURN: usize = 64;

/// A screenshot of an Incular window's rendered surface.
///
/// Pixels are tightly packed, top-to-bottom, straight-alpha RGBA8 values.
/// This is an application frame, not a capture of the desktop or another
/// application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screenshot {
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

impl Screenshot {
    /// Creates a screenshot from tightly packed RGBA8 pixels.
    pub fn from_rgba8(width: u32, height: u32, rgba8: Vec<u8>) -> Result<Self, SimulationError> {
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| {
                SimulationError::InvalidInput("screenshot dimensions overflow".into())
            })?;
        if expected == 0 || rgba8.len() != expected {
            return Err(SimulationError::InvalidInput(format!(
                "screenshot pixel length {} does not match {}x{} RGBA8",
                rgba8.len(),
                width,
                height
            )));
        }
        Ok(Self {
            width,
            height,
            rgba8,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the tightly packed RGBA8 pixel bytes.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.rgba8
    }

    /// Consumes the screenshot and returns its tightly packed RGBA8 pixels.
    #[must_use]
    pub fn into_pixels(self) -> Vec<u8> {
        self.rgba8
    }
}

/// Failure returned by an in-process simulation command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimulationError {
    /// The target application's event loop is not running, has stopped, or
    /// the application was dropped before the request was serviced.
    Disconnected,
    /// The target event loop did not service the request in time.
    Timeout,
    /// The requested retained window no longer exists.
    WindowNotFound(WindowId),
    /// The requested retained window was closed while the command was queued.
    WindowClosed(WindowId),
    /// No semantic node with the requested exact label was found.
    ElementNotFound(String),
    /// The command contained a value that cannot represent native input.
    InvalidInput(String),
    /// The renderer could not produce a window frame for a capture request.
    CaptureUnavailable(String),
    /// The runtime or renderer failed while servicing a frame request.
    FrameFailed(String),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => {
                formatter.write_str("the Incular simulation bridge is disconnected")
            }
            Self::Timeout => formatter.write_str("the Incular simulation request timed out"),
            Self::WindowNotFound(id) => write!(formatter, "simulation window {id} was not found"),
            Self::WindowClosed(id) => write!(formatter, "simulation window {id} was closed"),
            Self::ElementNotFound(label) => {
                write!(formatter, "no semantic element labeled {label:?} was found")
            }
            Self::InvalidInput(message) => write!(formatter, "invalid simulation input: {message}"),
            Self::CaptureUnavailable(message) => {
                write!(formatter, "window capture unavailable: {message}")
            }
            Self::FrameFailed(message) => write!(formatter, "simulation frame failed: {message}"),
        }
    }
}

impl std::error::Error for SimulationError {}

type Reply<T> = mpsc::SyncSender<Result<T, SimulationError>>;

pub(crate) enum SimulationRequest {
    Input {
        window_id: WindowId,
        event: InputEvent,
        reply: Reply<()>,
    },
    ClickAt {
        window_id: WindowId,
        position: Offset,
        reply: Reply<()>,
    },
    ClickLabel {
        window_id: WindowId,
        label: String,
        reply: Reply<()>,
    },
    WaitForFrame {
        window_id: WindowId,
        reply: Reply<()>,
    },
    Capture {
        window_id: WindowId,
        reply: Reply<Screenshot>,
    },
}

pub(crate) struct SimulationBridge {
    sender: mpsc::Sender<SimulationRequest>,
    wake: Mutex<Option<Arc<dyn super::RuntimeWake>>>,
    accepting: AtomicBool,
}

impl SimulationBridge {
    pub(crate) fn new(sender: mpsc::Sender<SimulationRequest>) -> Self {
        Self {
            sender,
            wake: Mutex::new(None),
            accepting: AtomicBool::new(true),
        }
    }

    fn enqueue(&self, request: SimulationRequest) -> Result<(), SimulationError> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err(SimulationError::Disconnected);
        }
        self.sender
            .send(request)
            .map_err(|_| SimulationError::Disconnected)?;
        if let Some(wake) = self.wake.lock().expect("simulation wake mutex").as_ref() {
            wake.wake();
        }
        Ok(())
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn super::RuntimeWake>) {
        *self.wake.lock().expect("simulation wake mutex") = Some(wake);
    }

    pub(crate) fn stop(&self) {
        self.accepting.store(false, Ordering::Release);
    }
}

#[derive(Default)]
struct KeyboardState {
    modifiers: Modifiers,
    pressed: HashSet<Code>,
}

/// A cloneable in-process controller for one Incular application window.
///
/// Calls block until the application's UI thread services the command. Use
/// this handle from a test/automation thread, not from an application build or
/// event callback, because those already run on the UI thread.
#[derive(Clone)]
pub struct Simulation {
    bridge: Arc<SimulationBridge>,
    window_id: WindowId,
    keyboard: Arc<Mutex<KeyboardState>>,
}

impl Simulation {
    pub(crate) fn new(bridge: Arc<SimulationBridge>, window_id: WindowId) -> Self {
        Self {
            bridge,
            window_id,
            keyboard: Arc::new(Mutex::new(KeyboardState::default())),
        }
    }

    /// Returns a controller targeting the window represented by `handle`.
    #[must_use]
    pub fn window(&self, handle: &WindowHandle) -> Self {
        self.for_window(handle.id())
    }

    /// Returns a controller targeting a known Incular window ID.
    #[must_use]
    pub fn for_window(&self, window_id: WindowId) -> Self {
        Self {
            bridge: self.bridge.clone(),
            window_id,
            keyboard: self.keyboard.clone(),
        }
    }

    #[must_use]
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Returns the mouse command group for this window.
    #[must_use]
    pub fn mouse(&self) -> SimulationMouse {
        SimulationMouse {
            simulation: self.clone(),
        }
    }

    /// Returns the keyboard command group for this window.
    #[must_use]
    pub fn keyboard(&self) -> SimulationKeyboard {
        SimulationKeyboard {
            simulation: self.clone(),
        }
    }

    /// Clicks the first enabled semantic node whose label exactly matches
    /// `label`, using move → down → up through the ordinary pointer pipeline.
    pub fn click(&self, label: impl Into<String>) -> Result<(), SimulationError> {
        self.mouse().click(label)
    }

    pub fn click_at(&self, position: Offset) -> Result<(), SimulationError> {
        self.mouse().click_at(position)
    }

    pub fn move_mouse_to(&self, position: Offset) -> Result<(), SimulationError> {
        self.mouse().move_to(position)
    }

    pub fn scroll(&self, delta: Offset) -> Result<(), SimulationError> {
        self.mouse().scroll(delta)
    }

    pub fn key_down(&self, code: Code) -> Result<(), SimulationError> {
        self.keyboard().key_down(code)
    }

    pub fn key_up(&self, code: Code) -> Result<(), SimulationError> {
        self.keyboard().key_up(code)
    }

    pub fn press(&self, code: Code) -> Result<(), SimulationError> {
        self.keyboard().press(code)
    }

    pub fn press_key(&self, key: KeyboardKey, code: Code) -> Result<(), SimulationError> {
        self.keyboard().press_key(key, code)
    }

    pub fn type_text(&self, text: impl Into<String>) -> Result<(), SimulationError> {
        self.keyboard().type_text(text)
    }

    /// Waits until the next frame for this window has been presented.
    pub fn wait_for_frame(&self) -> Result<(), SimulationError> {
        self.request(|reply| SimulationRequest::WaitForFrame {
            window_id: self.window_id,
            reply,
        })
    }

    /// Requests and returns the next rendered frame for this window.
    pub fn capture(&self) -> Result<Screenshot, SimulationError> {
        self.request(|reply| SimulationRequest::Capture {
            window_id: self.window_id,
            reply,
        })
    }

    /// Releases every modifier/key held by this simulation keyboard.
    pub fn release_all_keys(&self) -> Result<(), SimulationError> {
        self.keyboard().release_all()
    }

    fn request<T>(
        &self,
        build: impl FnOnce(Reply<T>) -> SimulationRequest,
    ) -> Result<T, SimulationError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.bridge.enqueue(build(sender))?;
        match receiver.recv_timeout(SIMULATION_REPLY_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(SimulationError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(SimulationError::Disconnected),
        }
    }

    fn input(&self, event: InputEvent) -> Result<(), SimulationError> {
        self.request(|reply| SimulationRequest::Input {
            window_id: self.window_id,
            event,
            reply,
        })
    }
}

/// Mouse/pointer operations for a [`Simulation`] window. Only the primary
/// pointer/left-button path is exposed here; multi-pointer tests can use the
/// lower-level runtime input API directly.
#[derive(Clone)]
pub struct SimulationMouse {
    simulation: Simulation,
}

impl SimulationMouse {
    pub fn move_to(&self, position: Offset) -> Result<(), SimulationError> {
        self.simulation.input(InputEvent::Pointer {
            phase: PointerPhase::Move,
            position,
        })
    }

    pub fn down(&self, position: Offset) -> Result<(), SimulationError> {
        self.simulation.input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position,
        })
    }

    pub fn up(&self, position: Offset) -> Result<(), SimulationError> {
        self.simulation.input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position,
        })
    }

    pub fn click(&self, label: impl Into<String>) -> Result<(), SimulationError> {
        self.simulation
            .request(|reply| SimulationRequest::ClickLabel {
                window_id: self.simulation.window_id,
                label: label.into(),
                reply,
            })
    }

    pub fn click_at(&self, position: Offset) -> Result<(), SimulationError> {
        self.simulation.request(|reply| SimulationRequest::ClickAt {
            window_id: self.simulation.window_id,
            position,
            reply,
        })
    }

    pub fn scroll(&self, delta: Offset) -> Result<(), SimulationError> {
        self.simulation.input(InputEvent::Scroll { delta })
    }
}

/// Keyboard operations for a [`Simulation`] window. Key events use Incular's
/// platform-neutral physical [`Code`] plus the logical key supplied to
/// [`Self::press_key`]. Text entry is a separate committed-text event, just as
/// it is for native IME-aware adapters.
#[derive(Clone)]
pub struct SimulationKeyboard {
    simulation: Simulation,
}

impl SimulationKeyboard {
    pub fn key_down(&self, code: Code) -> Result<(), SimulationError> {
        self.send_key(KeyState::Down, None, code)
    }

    pub fn key_up(&self, code: Code) -> Result<(), SimulationError> {
        self.send_key(KeyState::Up, None, code)
    }

    pub fn press(&self, code: Code) -> Result<(), SimulationError> {
        self.key_down(code)?;
        self.key_up(code)
    }

    pub fn press_key(&self, key: KeyboardKey, code: Code) -> Result<(), SimulationError> {
        self.send_key(KeyState::Down, Some(key.clone()), code)?;
        self.send_key(KeyState::Up, Some(key), code)
    }

    pub fn type_text(&self, text: impl Into<String>) -> Result<(), SimulationError> {
        let text = text.into();
        if text.chars().any(char::is_control) {
            return Err(SimulationError::InvalidInput(
                "type_text accepts printable committed text; use press for commands".into(),
            ));
        }
        self.simulation.input(InputEvent::Text(text))
    }

    pub fn release_all(&self) -> Result<(), SimulationError> {
        let pressed = self
            .simulation
            .keyboard
            .lock()
            .expect("simulation keyboard mutex")
            .pressed
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for code in pressed {
            self.key_up(code)?;
        }
        Ok(())
    }

    fn send_key(
        &self,
        state: KeyState,
        explicit_key: Option<KeyboardKey>,
        code: Code,
    ) -> Result<(), SimulationError> {
        let (key, modifiers, location, repeat, was_pressed) = {
            let mut keyboard = self
                .simulation
                .keyboard
                .lock()
                .expect("simulation keyboard mutex");
            let was_pressed = keyboard.pressed.contains(&code);
            if state.is_down() {
                if let Some(modifier) = modifier_for_code(code) {
                    keyboard.modifiers.insert(modifier);
                }
                keyboard.pressed.insert(code);
            }
            let values = (
                explicit_key.unwrap_or_else(|| logical_key(code)),
                keyboard.modifiers,
                location_for_code(code),
                state.is_down() && was_pressed,
                was_pressed,
            );
            if state.is_up() {
                // Key-up carries the modifier as pressed, then the local
                // simulator state releases it after the event is delivered.
                keyboard.pressed.remove(&code);
                if let Some(modifier) = modifier_for_code(code) {
                    keyboard.modifiers.remove(modifier);
                }
            }
            values
        };
        let event = KeyboardEvent {
            state,
            key,
            code,
            location,
            modifiers,
            repeat,
            is_composing: false,
        };
        let result = self.simulation.input(InputEvent::Key(event));
        if result.is_err() {
            // Keep local key state usable after a disconnected/failed request.
            let mut keyboard = self
                .simulation
                .keyboard
                .lock()
                .expect("simulation keyboard mutex");
            if state.is_down() {
                if !was_pressed {
                    keyboard.pressed.remove(&code);
                }
            } else {
                keyboard.pressed.insert(code);
            }
            keyboard.modifiers = modifiers_for_pressed(&keyboard.pressed);
        }
        result
    }
}

fn modifier_for_code(code: Code) -> Option<Modifiers> {
    match code {
        Code::AltLeft | Code::AltRight => Some(Modifiers::ALT),
        Code::ControlLeft | Code::ControlRight => Some(Modifiers::CONTROL),
        Code::MetaLeft | Code::MetaRight => Some(Modifiers::META),
        Code::ShiftLeft | Code::ShiftRight => Some(Modifiers::SHIFT),
        _ => None,
    }
}

fn modifiers_for_pressed(pressed: &HashSet<Code>) -> Modifiers {
    pressed
        .iter()
        .fold(Modifiers::default(), |mut modifiers, code| {
            if let Some(modifier) = modifier_for_code(*code) {
                modifiers.insert(modifier);
            }
            modifiers
        })
}

fn location_for_code(code: Code) -> Location {
    match code {
        Code::AltLeft | Code::ControlLeft | Code::MetaLeft | Code::ShiftLeft => Location::Left,
        Code::AltRight | Code::ControlRight | Code::MetaRight | Code::ShiftRight => Location::Right,
        Code::NumpadEnter => Location::Numpad,
        _ => Location::Standard,
    }
}

fn logical_key(code: Code) -> KeyboardKey {
    let character = match code {
        Code::Backquote => Some("`"),
        Code::Backslash => Some("\\"),
        Code::BracketLeft => Some("["),
        Code::BracketRight => Some("]"),
        Code::Comma => Some(","),
        Code::Digit0 => Some("0"),
        Code::Digit1 => Some("1"),
        Code::Digit2 => Some("2"),
        Code::Digit3 => Some("3"),
        Code::Digit4 => Some("4"),
        Code::Digit5 => Some("5"),
        Code::Digit6 => Some("6"),
        Code::Digit7 => Some("7"),
        Code::Digit8 => Some("8"),
        Code::Digit9 => Some("9"),
        Code::Equal => Some("="),
        Code::KeyA => Some("a"),
        Code::KeyB => Some("b"),
        Code::KeyC => Some("c"),
        Code::KeyD => Some("d"),
        Code::KeyE => Some("e"),
        Code::KeyF => Some("f"),
        Code::KeyG => Some("g"),
        Code::KeyH => Some("h"),
        Code::KeyI => Some("i"),
        Code::KeyJ => Some("j"),
        Code::KeyK => Some("k"),
        Code::KeyL => Some("l"),
        Code::KeyM => Some("m"),
        Code::KeyN => Some("n"),
        Code::KeyO => Some("o"),
        Code::KeyP => Some("p"),
        Code::KeyQ => Some("q"),
        Code::KeyR => Some("r"),
        Code::KeyS => Some("s"),
        Code::KeyT => Some("t"),
        Code::KeyU => Some("u"),
        Code::KeyV => Some("v"),
        Code::KeyW => Some("w"),
        Code::KeyX => Some("x"),
        Code::KeyY => Some("y"),
        Code::KeyZ => Some("z"),
        Code::Minus => Some("-"),
        Code::Period => Some("."),
        Code::Quote => Some("'"),
        Code::Semicolon => Some(";"),
        Code::Slash => Some("/"),
        Code::Space => Some(" "),
        Code::Numpad0 => Some("0"),
        Code::Numpad1 => Some("1"),
        Code::Numpad2 => Some("2"),
        Code::Numpad3 => Some("3"),
        Code::Numpad4 => Some("4"),
        Code::Numpad5 => Some("5"),
        Code::Numpad6 => Some("6"),
        Code::Numpad7 => Some("7"),
        Code::Numpad8 => Some("8"),
        Code::Numpad9 => Some("9"),
        Code::NumpadAdd => Some("+"),
        Code::NumpadComma => Some(","),
        Code::NumpadDecimal => Some("."),
        Code::NumpadDivide => Some("/"),
        Code::NumpadEqual => Some("="),
        Code::NumpadMultiply | Code::NumpadStar => Some("*"),
        Code::NumpadSubtract => Some("-"),
        _ => None,
    };
    if let Some(character) = character {
        return KeyboardKey::Character(character.into());
    }
    let named = match code {
        Code::AltLeft | Code::AltRight => NamedKey::Alt,
        Code::CapsLock => NamedKey::CapsLock,
        Code::ControlLeft | Code::ControlRight => NamedKey::Control,
        Code::Enter | Code::NumpadEnter => NamedKey::Enter,
        Code::Escape => NamedKey::Escape,
        Code::MetaLeft | Code::MetaRight => NamedKey::Meta,
        Code::NumLock => NamedKey::NumLock,
        Code::ScrollLock => NamedKey::ScrollLock,
        Code::ShiftLeft | Code::ShiftRight => NamedKey::Shift,
        Code::Tab => NamedKey::Tab,
        Code::Backspace => NamedKey::Backspace,
        Code::Delete => NamedKey::Delete,
        Code::Insert => NamedKey::Insert,
        Code::PageDown => NamedKey::PageDown,
        Code::PageUp => NamedKey::PageUp,
        Code::ArrowDown => NamedKey::ArrowDown,
        Code::ArrowLeft => NamedKey::ArrowLeft,
        Code::ArrowRight => NamedKey::ArrowRight,
        Code::ArrowUp => NamedKey::ArrowUp,
        Code::End => NamedKey::End,
        Code::Home => NamedKey::Home,
        _ => NamedKey::Unidentified,
    };
    KeyboardKey::Named(named)
}

fn valid_offset(offset: Offset) -> Result<(), SimulationError> {
    if offset.x.is_finite() && offset.y.is_finite() {
        Ok(())
    } else {
        Err(SimulationError::InvalidInput(
            "pointer and scroll offsets must be finite".into(),
        ))
    }
}

impl Application {
    /// Services queued in-process simulation requests on the application UI
    /// thread. Native adapters call this from their event-loop wake path.
    pub fn process_simulation_requests(&mut self) {
        for _ in 0..MAX_SIMULATION_REQUESTS_PER_TURN {
            let request = match self.simulation_receiver.try_recv() {
                Ok(request) => request,
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            };
            self.handle_simulation_request(request);
        }
    }

    fn handle_simulation_request(&mut self, request: SimulationRequest) {
        match request {
            SimulationRequest::Input {
                window_id,
                event,
                reply,
            } => {
                let result = self
                    .ensure_simulation_layout(window_id)
                    .and_then(|_| self.dispatch_simulation_input(window_id, event));
                let _ = reply.send(result);
            }
            SimulationRequest::ClickAt {
                window_id,
                position,
                reply,
            } => {
                let _ = reply.send(self.dispatch_simulation_click(window_id, position));
            }
            SimulationRequest::ClickLabel {
                window_id,
                label,
                reply,
            } => {
                let trigger = format!("click({label:?})");
                let result = self
                    .simulation_label_bounds(window_id, &label)
                    .and_then(|bounds| {
                        self.dispatch_simulation_click_prepared(
                            window_id,
                            bounds.center(),
                            &trigger,
                        )
                    });
                let _ = reply.send(result);
            }
            SimulationRequest::WaitForFrame { window_id, reply } => {
                if self.contains_window(window_id) {
                    let _ =
                        self.with_window_mut(window_id, |record| record.runtime.request_frame());
                    self.simulation_frame_waiters
                        .entry(window_id)
                        .or_default()
                        .push(reply);
                } else {
                    let _ = reply.send(Err(SimulationError::WindowNotFound(window_id)));
                }
            }
            SimulationRequest::Capture { window_id, reply } => {
                if self.contains_window(window_id) {
                    let _ =
                        self.with_window_mut(window_id, |record| record.runtime.request_frame());
                    self.simulation_capture_waiters
                        .entry(window_id)
                        .or_default()
                        .push(reply);
                } else {
                    let _ = reply.send(Err(SimulationError::WindowNotFound(window_id)));
                }
            }
        }
    }

    fn dispatch_simulation_input(
        &mut self,
        window_id: WindowId,
        event: InputEvent,
    ) -> Result<(), SimulationError> {
        let trigger = format!("simulation::{}", super::diagnostic_input_trigger(&event));
        self.dispatch_simulation_input_with_trigger(window_id, event, &trigger)
    }

    fn dispatch_simulation_input_with_trigger(
        &mut self,
        window_id: WindowId,
        event: InputEvent,
        trigger: &str,
    ) -> Result<(), SimulationError> {
        if let InputEvent::Pointer { position, .. } | InputEvent::PointerWithId { position, .. } =
            &event
        {
            valid_offset(*position)?;
        }
        if let InputEvent::Scroll { delta } = &event {
            valid_offset(*delta)?;
        }
        if !self.contains_window(window_id) {
            return Err(SimulationError::WindowNotFound(window_id));
        }
        let applied = self.with_window_mut(window_id, |record| {
            record.input_events = record.input_events.wrapping_add(1);
            let _ = record.runtime.handle_input_with_trigger(event, trigger);
        });
        applied
            .map(|_| ())
            .ok_or(SimulationError::WindowNotFound(window_id))
    }

    fn dispatch_simulation_click(
        &mut self,
        window_id: WindowId,
        position: Offset,
    ) -> Result<(), SimulationError> {
        valid_offset(position)?;
        self.ensure_simulation_layout(window_id)?;
        self.dispatch_simulation_click_prepared(
            window_id,
            position,
            &format!("click_at({:.1}, {:.1})", position.x, position.y),
        )
    }

    fn dispatch_simulation_click_prepared(
        &mut self,
        window_id: WindowId,
        position: Offset,
        trigger: &str,
    ) -> Result<(), SimulationError> {
        self.dispatch_simulation_input_with_trigger(
            window_id,
            InputEvent::Pointer {
                phase: PointerPhase::Move,
                position,
            },
            trigger,
        )?;
        self.dispatch_simulation_input_with_trigger(
            window_id,
            InputEvent::Pointer {
                phase: PointerPhase::Down,
                position,
            },
            trigger,
        )?;
        self.dispatch_simulation_input_with_trigger(
            window_id,
            InputEvent::Pointer {
                phase: PointerPhase::Up,
                position,
            },
            trigger,
        )
    }

    fn ensure_simulation_layout(&mut self, window_id: WindowId) -> Result<(), SimulationError> {
        let diagnostics = self
            .window_diagnostics(window_id)
            .ok_or(SimulationError::WindowNotFound(window_id))?;
        if diagnostics.physical_size.is_zero() {
            return Err(SimulationError::FrameFailed(
                "the target window has a zero-sized surface".into(),
            ));
        }
        self.run_window_frame_at(
            window_id,
            Constraints::tight(diagnostics.logical_size),
            Instant::now(),
        )
        .map_err(|error| SimulationError::FrameFailed(format!("{error:?}")))?;
        // The preparation frame is not presented by this runtime-only step;
        // leave the native adapter a real redraw to present afterward.
        let _ = self.with_window_mut(window_id, |record| record.runtime.request_frame());
        Ok(())
    }

    fn simulation_label_bounds(
        &mut self,
        window_id: WindowId,
        label: &str,
    ) -> Result<Rect, SimulationError> {
        self.ensure_simulation_layout(window_id)?;
        let viewport = self
            .window_diagnostics(window_id)
            .map(|diagnostics| Rect::from_origin_size(Offset::ZERO, diagnostics.logical_size))
            .ok_or(SimulationError::WindowNotFound(window_id))?;
        let result = self.registry.borrow().get(window_id).and_then(|record| {
            let mut fallback = None;
            record
                .runtime
                .tree()
                .semantics()
                .iter()
                .find_map(|(_, node)| {
                    if node.label.as_deref() != Some(label) || !node.state.enabled {
                        return None;
                    }
                    // Semantic trees can retain children just outside a
                    // viewport (especially sliver children). Resolve a
                    // pointer target only from the visible intersection so a
                    // simulation never reports a click that the pointer
                    // pipeline cannot receive.
                    let visible_bounds = node.bounds.intersection(viewport)?;
                    if visible_bounds.size.width <= 0.0 || visible_bounds.size.height <= 0.0 {
                        return None;
                    }
                    if node.actions.contains(&SemanticActionKind::Activate) {
                        return Some(visible_bounds);
                    }
                    fallback = Some(visible_bounds);
                    None
                })
                .or(fallback)
        });
        result.ok_or_else(|| SimulationError::ElementNotFound(label.to_owned()))
    }

    pub fn simulation_capture_pending(&self, window_id: WindowId) -> bool {
        self.simulation_capture_waiters
            .get(&window_id)
            .is_some_and(|waiters| !waiters.is_empty())
    }

    /// Reports whether a simulator is waiting for the next presented frame.
    #[must_use]
    pub fn simulation_frame_pending(&self, window_id: WindowId) -> bool {
        self.simulation_frame_waiters
            .get(&window_id)
            .is_some_and(|waiters| !waiters.is_empty())
    }

    /// Completes simulator waiters after the native renderer has presented a
    /// frame. `capture` is supplied only when a renderer serviced a pending
    /// capture request.
    pub fn complete_simulation_frame(
        &mut self,
        window_id: WindowId,
        presented: bool,
        capture: Option<Result<Screenshot, String>>,
    ) {
        if !presented {
            return;
        }
        if let Some(waiters) = self.simulation_frame_waiters.remove(&window_id) {
            for reply in waiters {
                let _ = reply.send(Ok(()));
            }
        }
        if let Some(waiters) = self.simulation_capture_waiters.remove(&window_id) {
            let result = capture
                .map(|result| result.map_err(SimulationError::CaptureUnavailable))
                .unwrap_or_else(|| {
                    Err(SimulationError::CaptureUnavailable(
                        "the renderer did not return a capture".into(),
                    ))
                });
            for reply in waiters {
                let _ = reply.send(result.clone());
            }
        }
    }

    /// Fails simulator requests when a native frame cannot be produced.
    pub fn fail_simulation_frame(&mut self, window_id: WindowId, message: impl Into<String>) {
        let error = SimulationError::FrameFailed(message.into());
        if let Some(waiters) = self.simulation_frame_waiters.remove(&window_id) {
            for reply in waiters {
                let _ = reply.send(Err(error.clone()));
            }
        }
        if let Some(waiters) = self.simulation_capture_waiters.remove(&window_id) {
            let capture_error = SimulationError::CaptureUnavailable(error.to_string());
            for reply in waiters {
                let _ = reply.send(Err(capture_error.clone()));
            }
        }
    }

    pub(crate) fn fail_simulation_window(&mut self, window_id: WindowId) {
        let error = SimulationError::WindowClosed(window_id);
        if let Some(waiters) = self.simulation_frame_waiters.remove(&window_id) {
            for reply in waiters {
                let _ = reply.send(Err(error.clone()));
            }
        }
        if let Some(waiters) = self.simulation_capture_waiters.remove(&window_id) {
            for reply in waiters {
                let _ = reply.send(Err(error.clone()));
            }
        }
    }

    pub(crate) fn stop_simulation(&self) {
        self.simulation_bridge.stop();
    }
}

trait RectCenter {
    fn center(self) -> Offset;
}

impl RectCenter for Rect {
    fn center(self) -> Offset {
        Offset::new(
            self.origin.x + self.size.width * 0.5,
            self.origin.y + self.size.height * 0.5,
        )
    }
}

// Keep the imported input types intentionally visible in this module's API
// documentation and make accidental future requests for native IME payloads
// explicit at the type boundary.
