/// Renderer-neutral Material icon catalog bridge. Applications may use any
/// `IconData` from `incular-text`; these stable names are convenient for the
/// common desktop glyphs and remain text based until an icon font is selected.
pub struct Icons;

#[allow(non_upper_case_globals)]
impl Icons {
    pub const ADD: &'static str = "+";
    pub const add: &'static str = Self::ADD;
    pub const CLOSE: &'static str = "×";
    pub const close: &'static str = Self::CLOSE;
    pub const MENU: &'static str = "☰";
    pub const menu: &'static str = Self::MENU;
    pub const ARROW_BACK: &'static str = "‹";
    pub const arrow_back: &'static str = Self::ARROW_BACK;
    pub const ARROW_FORWARD: &'static str = "›";
    pub const arrow_forward: &'static str = Self::ARROW_FORWARD;
    pub const CHECK: &'static str = "✓";
    pub const check: &'static str = Self::CHECK;
    pub const SETTINGS: &'static str = "⚙";
    pub const settings: &'static str = Self::SETTINGS;
    pub const SEARCH: &'static str = "⌕";
    pub const search: &'static str = Self::SEARCH;
    pub const MORE_VERT: &'static str = "⋮";
    pub const more_vert: &'static str = Self::MORE_VERT;
    pub const ARROW_DROP_DOWN: &'static str = "⌄";
    pub const arrow_drop_down: &'static str = Self::ARROW_DROP_DOWN;
    pub const ARROW_DROP_UP: &'static str = "⌃";
    pub const arrow_drop_up: &'static str = Self::ARROW_DROP_UP;
    pub const CHEVRON_LEFT: &'static str = "‹";
    pub const chevron_left: &'static str = Self::CHEVRON_LEFT;
    pub const CHEVRON_RIGHT: &'static str = "›";
    pub const chevron_right: &'static str = Self::CHEVRON_RIGHT;
    pub const DELETE: &'static str = "⌫";
    pub const delete: &'static str = Self::DELETE;
    pub const EDIT: &'static str = "✎";
    pub const edit: &'static str = Self::EDIT;
    pub const REFRESH: &'static str = "↻";
    pub const refresh: &'static str = Self::REFRESH;
    pub const PLAY_ARROW: &'static str = "▶";
    pub const play_arrow: &'static str = Self::PLAY_ARROW;
    pub const PAUSE: &'static str = "Ⅱ";
    pub const pause: &'static str = Self::PAUSE;
    pub const INFO_OUTLINE: &'static str = "ⓘ";
    pub const info_outline: &'static str = Self::INFO_OUTLINE;
    pub const HELP_OUTLINE: &'static str = "?";
    pub const help_outline: &'static str = Self::HELP_OUTLINE;
    pub const WARNING: &'static str = "⚠";
    pub const warning: &'static str = Self::WARNING;
}
