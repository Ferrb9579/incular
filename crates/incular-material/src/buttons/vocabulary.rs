#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonTextTheme {
    #[default]
    Normal,
    Accent,
    Primary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonBarLayoutBehavior {
    #[default]
    Constrained,
    Padded,
}

pub const K_FLOATING_ACTION_BUTTON_MARGIN: f32 = 16.0;
