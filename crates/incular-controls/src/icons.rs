//! Small retained vector icon set used by controls. This is intentionally not
//! an icon-font or a general-purpose icon catalogue.

use incular_core::Color;
use incular_widgets::{Icon, Widget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlIcon {
    Check,
    Close,
    Plus,
    Minus,
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
}

impl ControlIcon {
    #[must_use]
    pub fn path(self) -> std::sync::Arc<incular_rendering::Path> {
        match self {
            Self::Check => incular_widgets::icons::check(),
            Self::Close => incular_widgets::icons::close(),
            Self::Plus => incular_widgets::icons::plus(),
            Self::Minus => incular_widgets::icons::minus(),
            Self::ChevronDown => incular_widgets::icons::chevron_down(),
            Self::ChevronUp => incular_widgets::icons::chevron_up(),
            Self::ChevronLeft => incular_widgets::icons::chevron_left(),
            Self::ChevronRight => incular_widgets::icons::chevron_right(),
        }
    }

    #[must_use]
    pub fn widget(self, size: f32, color: Color) -> Widget {
        Icon::new(self.path()).size(size).brush(color).into()
    }
}

impl From<ControlIcon> for Widget {
    fn from(value: ControlIcon) -> Self {
        value.widget(16., Color::WHITE)
    }
}
