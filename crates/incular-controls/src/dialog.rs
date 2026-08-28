//! Dialog anatomy. Behavior is shared with the popup/overlay foundation.
pub use crate::popup::{
    Arrow, Close, Description, Popup, Portal, Positioner, Root, Title, Trigger,
};

/// Dialog anatomy names for applications that prefer the full compound API.
pub type Backdrop = Portal;
pub type Viewport = Popup;

/// Alert dialog uses the same retained dialog state with a stricter semantic
/// contract supplied by the application shell.
pub type AlertRoot = crate::alert_dialog::Root;

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn facade_aliases_preserve_dialog_composition_and_conversion() {
        let _: Widget = Root::new().open(true).child(Text::new("dialog")).into();
        let _: Widget = Backdrop::new(Text::new("backdrop")).open(true).into();
        let _: Widget = Viewport::new(Text::new("viewport")).into();
        let _: Widget = Trigger::new(Text::new("trigger")).into();
        let _: Widget = Close::new(Text::new("close")).into();
        let _: Widget = AlertRoot::new().open(true).child(Text::new("alert")).into();
    }
}
