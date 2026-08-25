//! Application commands, identifiers, and keyboard shortcut configurations.

use keyboard_types::{Code, Modifiers};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StudioCommand {
    NewFile,
    OpenFile,
    SaveFile,
    CloseTab,
    NextTab,
    PrevTab,
    ToggleSidebar,
    ToggleInspector,
    ToggleBottomPanel,
    CommandPalette,
    OpenSettings,
    ToggleTheme,
    ToggleWrap,
    NewWindow,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    FindInWorkspace,
}

impl StudioCommand {
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::NewFile => "File: New File",
            Self::OpenFile => "File: Open File...",
            Self::SaveFile => "File: Save",
            Self::CloseTab => "View: Close Active Tab",
            Self::NextTab => "View: Next Tab",
            Self::PrevTab => "View: Previous Tab",
            Self::ToggleSidebar => "View: Toggle Primary Side Bar",
            Self::ToggleInspector => "View: Toggle Inspector",
            Self::ToggleBottomPanel => "View: Toggle Bottom Panel",
            Self::CommandPalette => "View: Show Command Palette",
            Self::OpenSettings => "Preferences: Open Settings",
            Self::ToggleTheme => "Preferences: Toggle Dark/Light Theme",
            Self::ToggleWrap => "Editor: Toggle Word Wrap",
            Self::NewWindow => "Window: New Window",
            Self::ZoomIn => "View: Zoom In",
            Self::ZoomOut => "View: Zoom Out",
            Self::ResetZoom => "View: Reset Zoom",
            Self::FindInWorkspace => "Search: Find in Workspace",
        }
    }

    #[must_use]
    pub fn shortcut_display(&self) -> &'static str {
        match self {
            Self::NewFile => "Ctrl+N",
            Self::OpenFile => "Ctrl+O",
            Self::SaveFile => "Ctrl+S",
            Self::CloseTab => "Ctrl+W",
            Self::NextTab => "Ctrl+PageDown",
            Self::PrevTab => "Ctrl+PageUp",
            Self::ToggleSidebar => "Ctrl+B",
            Self::ToggleInspector => "Ctrl+Alt+I",
            Self::ToggleBottomPanel => "Ctrl+J",
            Self::CommandPalette => "Ctrl+Shift+P",
            Self::OpenSettings => "Ctrl+,",
            Self::ToggleTheme => "Ctrl+K Ctrl+T",
            Self::ToggleWrap => "Alt+Z",
            Self::NewWindow => "Ctrl+Shift+N",
            Self::ZoomIn => "Ctrl+=",
            Self::ZoomOut => "Ctrl+-",
            Self::ResetZoom => "Ctrl+0",
            Self::FindInWorkspace => "Ctrl+Shift+F",
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn matches_key(&self, code: Code, modifiers: Modifiers) -> bool {
        let ctrl = modifiers.contains(Modifiers::CONTROL);
        let shift = modifiers.contains(Modifiers::SHIFT);
        let alt = modifiers.contains(Modifiers::ALT);

        match self {
            Self::NewFile => ctrl && !shift && !alt && code == Code::KeyN,
            Self::OpenFile => ctrl && !shift && !alt && code == Code::KeyO,
            Self::SaveFile => ctrl && !shift && !alt && code == Code::KeyS,
            Self::CloseTab => ctrl && !shift && !alt && code == Code::KeyW,
            Self::ToggleSidebar => ctrl && !shift && !alt && code == Code::KeyB,
            Self::ToggleBottomPanel => ctrl && !shift && !alt && code == Code::KeyJ,
            Self::CommandPalette => ctrl && shift && !alt && code == Code::KeyP,
            Self::OpenSettings => ctrl && !shift && !alt && code == Code::Comma,
            Self::ToggleWrap => !ctrl && !shift && alt && code == Code::KeyZ,
            Self::NewWindow => ctrl && shift && !alt && code == Code::KeyN,
            Self::ZoomIn => ctrl && (code == Code::Equal || code == Code::NumpadAdd),
            Self::ZoomOut => ctrl && (code == Code::Minus || code == Code::NumpadSubtract),
            Self::ResetZoom => ctrl && (code == Code::Digit0 || code == Code::Numpad0),
            Self::FindInWorkspace => ctrl && shift && !alt && code == Code::KeyF,
            _ => false,
        }
    }
}
