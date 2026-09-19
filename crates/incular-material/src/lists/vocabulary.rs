#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListTileControlAffinity {
    #[default]
    Platform,
    Leading,
    Trailing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListTileStyle {
    #[default]
    List,
    Drawer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListTileTitleAlignment {
    ThreeLine,
    #[default]
    TitleHeight,
    Top,
    Center,
    Bottom,
}
