//! Reactive state model for Incular Studio.
//! Uses fine-grained reactive Signals rather than a monolithic global Mutex.

use incular_runtime::Signal;
use incular_widgets::internal::TextEditingController;
use incular_widgets::{FocusNode, ScrollController};
use std::{collections::HashSet, sync::Arc};

use crate::{localization::StudioLocale, restoration::RestorationData, theme::ThemeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BottomTab {
    #[default]
    Search,
    Problems,
    Output,
    Canvas,
}

#[derive(Clone, Debug)]
pub struct TreeNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub depth: usize,
    pub icon: &'static str,
    pub children_count: usize,
}

#[derive(Clone)]
pub struct DocumentTab {
    pub id: String,
    pub title: String,
    pub path: String,
    pub is_dirty: Signal<bool>,
    pub controller: TextEditingController,
    pub scroll_controller: ScrollController,
    pub focus_node: FocusNode,
    pub cursor_line: Signal<usize>,
    pub cursor_col: Signal<usize>,
    pub word_wrap: Signal<bool>,
}

impl DocumentTab {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        path: impl Into<String>,
        initial_text: &str,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            path: path.into(),
            is_dirty: Signal::new(false),
            controller: TextEditingController::with_text(initial_text),
            scroll_controller: ScrollController::new(),
            focus_node: FocusNode::new(),
            cursor_line: Signal::new(1),
            cursor_col: Signal::new(1),
            word_wrap: Signal::new(false),
        }
    }
}

impl PartialEq for DocumentTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.title == other.title && self.path == other.path
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub file: String,
    pub line_number: usize,
    pub match_text: String,
    pub preview: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProblemItem {
    pub severity: ProblemSeverity,
    pub file: String,
    pub line: usize,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone)]
pub struct ProjectState {
    pub nodes: Arc<Vec<TreeNode>>,
    pub expanded_nodes: Signal<HashSet<String>>,
    pub selected_node: Signal<Option<String>>,
    pub filter_query: Signal<String>,
}

#[derive(Clone)]
pub struct EditorState {
    pub documents: Signal<Vec<DocumentTab>>,
    pub active_tab_index: Signal<usize>,
}

#[derive(Clone)]
pub struct SearchState {
    pub query: Signal<String>,
    pub controller: TextEditingController,
    pub results: Signal<Vec<SearchResult>>,
    pub is_searching: Signal<bool>,
    pub search_token: Signal<u64>,
}

#[derive(Clone)]
pub struct SettingsState {
    pub editor_font_size: Signal<f32>,
    pub tab_size: Signal<u32>,
    pub word_wrap: Signal<bool>,
    pub auto_save: Signal<bool>,
    pub form_error: Signal<Option<String>>,
}

#[derive(Clone)]
pub struct StudioState {
    pub sidebar_width: Signal<f32>,
    pub inspector_width: Signal<f32>,
    pub bottom_panel_height: Signal<f32>,
    pub sidebar_visible: Signal<bool>,
    pub inspector_visible: Signal<bool>,
    pub bottom_panel_visible: Signal<bool>,
    pub command_palette_open: Signal<bool>,
    pub command_palette_controller: TextEditingController,
    pub settings_open: Signal<bool>,
    pub theme_mode: Signal<ThemeMode>,
    pub locale: Signal<StudioLocale>,
    pub font_scale: Signal<f32>,
    pub status_message: Signal<String>,
    pub zoom_level: Signal<f32>,
    pub bottom_tab: Signal<BottomTab>,
    pub problems: Signal<Vec<ProblemItem>>,
    pub output_lines: Signal<Vec<String>>,
    pub project: ProjectState,
    pub editor: EditorState,
    pub search: SearchState,
    pub settings: SettingsState,
}

impl StudioState {
    #[must_use]
    pub fn new(restoration: &RestorationData, stress_tree_count: Option<usize>) -> Self {
        let theme_mode = match restoration.theme_mode.as_str() {
            "Light" => ThemeMode::Light,
            "HighContrast" => ThemeMode::HighContrast,
            _ => ThemeMode::Dark,
        };
        let locale = StudioLocale::from_code(&restoration.locale_code);

        // Populate sample or stress tree nodes
        let nodes = generate_project_nodes(stress_tree_count.unwrap_or(30));
        let mut initial_expanded = HashSet::new();
        initial_expanded.insert("root".to_owned());
        initial_expanded.insert("src".to_owned());

        let initial_docs = vec![
            DocumentTab::new(
                "main_rs",
                "main.rs",
                "src/main.rs",
                "// Welcome to Incular Studio!\n\nfn main() {\n    println!(\"Hello, Incular GUI!\");\n}\n\n// Demonstrating multi-lingual text:\n// English: Fast, safe, reactive UI\n// Arabic (RTL): مرحباً بكم في إنكيولار\n// Hindi: इनक्युलर में आपका स्वागत है\n// Japanese: Incularへようこそ\n",
            ),
            DocumentTab::new(
                "lib_rs",
                "lib.rs",
                "src/lib.rs",
                "pub mod state;\npub mod views;\npub mod theme;\n\n/// Returns framework version\npub fn version() -> &'static str {\n    \"0.1.0 (Flutter 3.47 Baseline)\"\n}\n",
            ),
            DocumentTab::new(
                "cargo_toml",
                "Cargo.toml",
                "Cargo.toml",
                "[package]\nname = \"incular-studio\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nincular = { path = \"../../crates/incular\" }\n",
            ),
        ];

        let initial_problems = vec![ProblemItem {
            severity: ProblemSeverity::Info,
            file: "src/main.rs".to_owned(),
            line: 3,
            message: "Incular Studio reference application initialized cleanly.".to_owned(),
        }];

        let initial_output = vec![
            "[info] Incular Studio initialized.".to_owned(),
            "[info] Retained element and damage reconciliation active.".to_owned(),
            "[info] Multi-window, focus, and command subsystem ready.".to_owned(),
        ];

        Self {
            sidebar_width: Signal::new(restoration.sidebar_width),
            inspector_width: Signal::new(restoration.inspector_width),
            bottom_panel_height: Signal::new(restoration.bottom_panel_height),
            sidebar_visible: Signal::new(restoration.sidebar_visible),
            inspector_visible: Signal::new(restoration.inspector_visible),
            bottom_panel_visible: Signal::new(restoration.bottom_panel_visible),
            command_palette_open: Signal::new(false),
            command_palette_controller: TextEditingController::new(),
            settings_open: Signal::new(false),
            theme_mode: Signal::new(theme_mode),
            locale: Signal::new(locale),
            font_scale: Signal::new(1.0),
            status_message: Signal::new("Ready".to_owned()),
            zoom_level: Signal::new(restoration.zoom_level),
            bottom_tab: Signal::new(BottomTab::Search),
            problems: Signal::new(initial_problems),
            output_lines: Signal::new(initial_output),
            project: ProjectState {
                nodes: Arc::new(nodes),
                expanded_nodes: Signal::new(initial_expanded),
                selected_node: Signal::new(Some("main_rs".to_owned())),
                filter_query: Signal::new(String::new()),
            },
            editor: EditorState {
                documents: Signal::new(initial_docs),
                active_tab_index: Signal::new(restoration.active_tab_index.min(2)),
            },
            search: SearchState {
                query: Signal::new(String::new()),
                controller: TextEditingController::new(),
                results: Signal::new(Vec::new()),
                is_searching: Signal::new(false),
                search_token: Signal::new(0),
            },
            settings: SettingsState {
                editor_font_size: Signal::new(restoration.editor_font_size),
                tab_size: Signal::new(restoration.tab_size),
                word_wrap: Signal::new(restoration.word_wrap),
                auto_save: Signal::new(restoration.auto_save),
                form_error: Signal::new(None),
            },
        }
    }

    #[must_use]
    pub fn active_document(&self) -> Option<DocumentTab> {
        let docs = self.editor.documents.get();
        let idx = self.editor.active_tab_index.get();
        docs.get(idx).cloned()
    }

    pub fn to_restoration(&self) -> RestorationData {
        RestorationData {
            sidebar_width: self.sidebar_width.get(),
            inspector_width: self.inspector_width.get(),
            bottom_panel_height: self.bottom_panel_height.get(),
            sidebar_visible: self.sidebar_visible.get(),
            inspector_visible: self.inspector_visible.get(),
            bottom_panel_visible: self.bottom_panel_visible.get(),
            active_tab_index: self.editor.active_tab_index.get(),
            theme_mode: format!("{:?}", self.theme_mode.get()),
            locale_code: self.locale.get().code().to_owned(),
            editor_font_size: self.settings.editor_font_size.get(),
            word_wrap: self.settings.word_wrap.get(),
            tab_size: self.settings.tab_size.get(),
            auto_save: self.settings.auto_save.get(),
            zoom_level: self.zoom_level.get(),
        }
    }
}

fn generate_project_nodes(count: usize) -> Vec<TreeNode> {
    let mut nodes = Vec::with_capacity(count + 10);
    nodes.push(TreeNode {
        id: "root".to_owned(),
        name: "incular-studio".to_owned(),
        path: ".".to_owned(),
        is_dir: true,
        depth: 0,
        icon: "📁",
        children_count: 5,
    });
    nodes.push(TreeNode {
        id: "src".to_owned(),
        name: "src".to_owned(),
        path: "src".to_owned(),
        is_dir: true,
        depth: 1,
        icon: "📁",
        children_count: 3,
    });
    nodes.push(TreeNode {
        id: "main_rs".to_owned(),
        name: "main.rs".to_owned(),
        path: "src/main.rs".to_owned(),
        is_dir: false,
        depth: 2,
        icon: "🦀",
        children_count: 0,
    });
    nodes.push(TreeNode {
        id: "lib_rs".to_owned(),
        name: "lib.rs".to_owned(),
        path: "src/lib.rs".to_owned(),
        is_dir: false,
        depth: 2,
        icon: "🦀",
        children_count: 0,
    });
    nodes.push(TreeNode {
        id: "state_rs".to_owned(),
        name: "state.rs".to_owned(),
        path: "src/state.rs".to_owned(),
        is_dir: false,
        depth: 2,
        icon: "🦀",
        children_count: 0,
    });
    nodes.push(TreeNode {
        id: "cargo_toml".to_owned(),
        name: "Cargo.toml".to_owned(),
        path: "Cargo.toml".to_owned(),
        is_dir: false,
        depth: 1,
        icon: "📦",
        children_count: 0,
    });
    nodes.push(TreeNode {
        id: "readme_md".to_owned(),
        name: "README.md".to_owned(),
        path: "README.md".to_owned(),
        is_dir: false,
        depth: 1,
        icon: "📝",
        children_count: 0,
    });

    // If stress count requested (e.g. 10,000 or 100,000 nodes):
    if count > nodes.len() {
        for i in nodes.len()..count {
            let is_dir = i % 10 == 0;
            nodes.push(TreeNode {
                id: format!("node_{i}"),
                name: if is_dir {
                    format!("module_{i}")
                } else {
                    format!("item_{i}.rs")
                },
                path: format!("src/generated/item_{i}.rs"),
                is_dir,
                depth: 1 + (i % 3),
                icon: if is_dir { "📁" } else { "📄" },
                children_count: if is_dir { 5 } else { 0 },
            });
        }
    }
    nodes
}
