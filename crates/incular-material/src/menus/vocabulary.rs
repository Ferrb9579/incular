#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropdownMenuCloseBehavior {
    #[default]
    All,
    SelfOnly,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopupMenuPosition {
    #[default]
    Over,
    Under,
}
