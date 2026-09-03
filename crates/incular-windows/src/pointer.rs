use incular_core::{
    NormalizedPressure, PointerDeviceKind, PointerSampleMetadata, StylusMetadata, StylusOrientation,
};
use incular_platform::NativePointerSample;
use std::{
    collections::{HashMap, VecDeque},
    mem::MaybeUninit,
    sync::{Arc, Mutex},
};
use windows_sys::Win32::UI::{
    Input::Pointer::{
        GetPointerFramePenInfoHistory, GetPointerPenInfo, POINTER_FLAG_INCONTACT, POINTER_PEN_INFO,
    },
    WindowsAndMessaging::{
        MSG, PEN_FLAG_BARREL, PEN_FLAG_ERASER, PEN_FLAG_INVERTED, PEN_MASK_PRESSURE,
        PEN_MASK_TILT_X, PEN_MASK_TILT_Y, PT_PEN, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE,
    },
};

const MAX_QUEUED_SAMPLES_PER_POINTER: usize = 256;

#[derive(Clone, Default)]
pub(crate) struct WindowsPointerState {
    inner: Arc<Mutex<WindowsPointerStateInner>>,
}

#[derive(Default)]
struct WindowsPointerStateInner {
    samples: HashMap<u64, VecDeque<NativePointerSample>>,
    devices: HashMap<usize, u64>,
    next_device: u64,
}

impl WindowsPointerStateInner {
    fn device_token(&mut self, native: usize) -> u64 {
        if let Some(token) = self.devices.get(&native).copied() {
            return token;
        }
        let token = self
            .next_device
            .checked_add(1)
            .expect("Windows pointer device token space exhausted");
        self.next_device = token;
        self.devices.insert(native, token);
        token
    }
}

impl WindowsPointerState {
    pub(crate) fn message_hook(&self) -> Box<dyn FnMut(*const std::ffi::c_void) -> bool + 'static> {
        let inner = self.inner.clone();
        Box::new(move |message| {
            if message.is_null() {
                return false;
            }
            // SAFETY: Winit's `with_msg_hook` contract supplies a valid borrowed
            // Win32 MSG for exactly this callback invocation.
            let message = unsafe { &*message.cast::<MSG>() };
            if !matches!(
                message.message,
                WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP
            ) {
                return false;
            }
            let pointer_id = u32::try_from(message.wParam & 0xffff)
                .expect("Win32 pointer ID is encoded in the low word");
            let captured = capture_pen_frame(pointer_id);
            if captured.is_empty() {
                return false;
            }
            let mut state = inner.lock().expect("Windows pen sample state");
            if message.message == WM_POINTERDOWN {
                state.samples.remove(&u64::from(pointer_id));
            }
            for (pointer, native_device, mut sample) in captured {
                sample.device = native_device.map(|device| state.device_token(device));
                let queue = state.samples.entry(pointer).or_default();
                while queue.len() >= MAX_QUEUED_SAMPLES_PER_POINTER {
                    queue.pop_front();
                }
                queue.push_back(sample);
            }
            // Observation only. Winit must continue processing WM_POINTER and
            // emit its ordinary WindowEvent::Touch sequence.
            false
        })
    }

    pub(crate) fn take(&self, pointer: u64) -> Option<NativePointerSample> {
        let mut state = self.inner.lock().expect("Windows pen sample state");
        let sample = state.samples.get_mut(&pointer)?.pop_front();
        if state.samples.get(&pointer).is_some_and(VecDeque::is_empty) {
            state.samples.remove(&pointer);
        }
        sample
    }
}

type CapturedPenSample = (u64, Option<usize>, NativePointerSample);

fn capture_pen_frame(pointer_id: u32) -> Vec<CapturedPenSample> {
    let mut entries = 0_u32;
    let mut pointers = 0_u32;
    // SAFETY: the first call requests only required counts and writes through
    // the two valid count pointers; no output buffer is supplied.
    let history_available = unsafe {
        GetPointerFramePenInfoHistory(
            pointer_id,
            &mut entries,
            &mut pointers,
            std::ptr::null_mut(),
        )
    } != 0;
    if history_available
        && let Some(count) = usize::try_from(entries).ok().and_then(|entries| {
            usize::try_from(pointers)
                .ok()
                .and_then(|pointers| entries.checked_mul(pointers))
        })
        && count > 0
    {
        let mut infos = Vec::<POINTER_PEN_INFO>::with_capacity(count);
        // SAFETY: `infos` has capacity for exactly entries*pointers structures.
        // Win32 writes those POD values synchronously and retains no pointer.
        if unsafe {
            GetPointerFramePenInfoHistory(
                pointer_id,
                &mut entries,
                &mut pointers,
                infos.as_mut_ptr(),
            )
        } != 0
        {
            // SAFETY: successful Win32 call initialized the count requested by
            // the returned entries/pointers dimensions. Clamp to capacity in
            // case a device reports inconsistent dimensions between calls.
            let initialized = usize::try_from(entries)
                .ok()
                .and_then(|entries| {
                    usize::try_from(pointers)
                        .ok()
                        .and_then(|pointers| entries.checked_mul(pointers))
                })
                .unwrap_or(0)
                .min(count);
            unsafe { infos.set_len(initialized) };
            let captured = infos
                .iter()
                .rev()
                .filter_map(|info| {
                    pen_sample(info).map(|(device, sample)| {
                        (u64::from(info.pointerInfo.pointerId), device, sample)
                    })
                })
                .collect::<Vec<_>>();
            if !captured.is_empty() {
                return captured;
            }
        }
    }

    let mut info = MaybeUninit::<POINTER_PEN_INFO>::uninit();
    // SAFETY: `info` is writable for the synchronous call and initialized only
    // when Win32 reports success.
    if unsafe { GetPointerPenInfo(pointer_id, info.as_mut_ptr()) } == 0 {
        return Vec::new();
    }
    let info = unsafe { info.assume_init() };
    pen_sample(&info)
        .map(|(device, sample)| vec![(u64::from(info.pointerInfo.pointerId), device, sample)])
        .unwrap_or_default()
}

fn pen_sample(info: &POINTER_PEN_INFO) -> Option<(Option<usize>, NativePointerSample)> {
    if info.pointerInfo.pointerType != PT_PEN {
        return None;
    }
    let inverted = info.penFlags & (PEN_FLAG_INVERTED | PEN_FLAG_ERASER) != 0;
    let pressure = (info.penMask & PEN_MASK_PRESSURE != 0)
        .then(|| NormalizedPressure::from_range(f64::from(info.pressure), 1024.0))
        .flatten();
    let orientation = (info.penMask & PEN_MASK_TILT_X != 0 && info.penMask & PEN_MASK_TILT_Y != 0)
        .then(|| StylusOrientation::from_tilt_degrees(f64::from(info.tiltX), f64::from(info.tiltY)))
        .flatten();
    let source = info.pointerInfo.sourceDevice;
    let device = (source != 0).then_some(source as usize);
    Some((
        device,
        NativePointerSample {
            // `sourceDevice` is a Win32 HANDLE. Never publish it as a portable
            // ID; the owning state maps it to a process-local opaque token.
            device: None,
            kind: if inverted {
                PointerDeviceKind::InvertedStylus
            } else {
                PointerDeviceKind::Stylus
            },
            sample: PointerSampleMetadata {
                pressure,
                stylus: Some(StylusMetadata {
                    orientation,
                    barrel_button: info.penFlags & PEN_FLAG_BARREL != 0,
                }),
            },
            in_contact: Some(info.pointerInfo.pointerFlags & POINTER_FLAG_INCONTACT != 0),
        },
    ))
}
