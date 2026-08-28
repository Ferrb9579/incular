//! Popover anatomy backed by the common popup positioner.
pub use crate::popup::{
    Arrow, Close, Description, Popup, Portal, Positioner, Root, Title, Trigger,
};

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn facade_parts_preserve_generic_content_and_widget_lowering() {
        let _: Widget = Root::new().open(true).child(Text::new("content")).into();
        let _: Widget = Trigger::new(Text::new("trigger")).into();
        let _: Widget = Portal::new(Text::new("anchor"))
            .overlay(Popup::new(Text::new("overlay")))
            .open(true)
            .into();
        let _: Widget = Positioner::new(Text::new("positioned")).into();
        let _: Widget = Arrow::new().child(Text::new("arrow")).into();
        let _: Widget = Title::new("title").into();
        let _: Widget = Description::new("description").into();
        let _: Widget = Close::new(Text::new("close")).into();
    }
}
