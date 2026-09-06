//! Popup-local positions are translated after native input normalization.
use incular_core::{InputEvent, Offset};
use incular_platform::PlatformEvent;
pub(crate) fn offset_transient_platform_event(
    event: PlatformEvent,
    offset: Offset,
) -> PlatformEvent {
    match event {
        PlatformEvent::Input(InputEvent::Pointer { phase, position }) => {
            PlatformEvent::Input(InputEvent::Pointer {
                phase,
                position: position + offset,
            })
        }
        PlatformEvent::Input(InputEvent::PointerWithId {
            pointer,
            phase,
            position,
        }) => PlatformEvent::Input(InputEvent::PointerWithId {
            pointer,
            phase,
            position: position + offset,
        }),
        PlatformEvent::Input(InputEvent::PointerWithMetadata {
            pointer,
            device,
            kind,
            buttons,
            button,
            sample,
            phase,
            position,
        }) => PlatformEvent::Input(InputEvent::PointerWithMetadata {
            pointer,
            device,
            kind,
            buttons,
            button,
            sample,
            phase,
            position: position + offset,
        }),
        other => other,
    }
}
