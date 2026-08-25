//! Context-menu naming facade. Context menus reuse the regular menu engine;
//! only the anchor trigger differs at the platform input layer.

pub use crate::menu::{
    Arrow, Close, Description, Group, Item, PopupRoot as Root, Portal, Positioner, Separator,
    Title, Trigger,
};
