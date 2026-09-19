#![allow(non_upper_case_globals)]

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerAlignment {
    #[default]
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollapseMode {
    #[default]
    Parallax,
    Pin,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StretchMode {
    #[default]
    ZoomBackground,
    BlurBackground,
    FadeTitle,
}

pub const K_TOOLBAR_HEIGHT: f32 = 56.0;

pub const k_toolbar_height: f32 = K_TOOLBAR_HEIGHT;
