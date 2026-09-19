#![allow(non_upper_case_globals)]

use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BottomNavigationBarType {
    #[default]
    Fixed,
    Shifting,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BottomNavigationBarLandscapeLayout {
    #[default]
    Spread,
    Centered,
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationDestinationLabelBehavior {
    #[default]
    AlwaysShow,
    AlwaysHide,
    OnlyShowSelected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationRailLabelType {
    #[default]
    None,
    Selected,
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabAlignment {
    Start,
    StartOffset,
    #[default]
    Fill,
    Center,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabBarIndicatorSize {
    #[default]
    Tab,
    Label,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabIndicatorAnimation {
    #[default]
    Linear,
    Elastic,
}

pub const K_BOTTOM_NAVIGATION_BAR_HEIGHT: f32 = 56.0;

pub const K_TAB_SCROLL_DURATION: Duration = Duration::from_millis(300);

pub const k_bottom_navigation_bar_height: f32 = K_BOTTOM_NAVIGATION_BAR_HEIGHT;
