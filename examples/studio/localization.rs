//! Internationalization and multi-lingual catalog for Incular Studio.
//! Supports English (LTR), Arabic (RTL), Hindi (LTR), and Japanese (LTR).

use incular_config::TextDirection;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StudioLocale {
    #[default]
    English,
    Arabic,
    Hindi,
    Japanese,
}

impl StudioLocale {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::Arabic => "ar-EG",
            Self::Hindi => "hi-IN",
            Self::Japanese => "ja-JP",
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::English => "English (US)",
            Self::Arabic => "العربية (Arabic)",
            Self::Hindi => "हिन्दी (Hindi)",
            Self::Japanese => "日本語 (Japanese)",
        }
    }

    #[must_use]
    pub fn direction(&self) -> TextDirection {
        match self {
            Self::Arabic => TextDirection::Rtl,
            _ => TextDirection::Ltr,
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            "ar-EG" | "ar" => Self::Arabic,
            "hi-IN" | "hi" => Self::Hindi,
            "ja-JP" | "ja" => Self::Japanese,
            _ => Self::English,
        }
    }
}

pub struct StudioStrings {
    pub app_title: &'static str,
    pub file_menu: &'static str,
    pub edit_menu: &'static str,
    pub view_menu: &'static str,
    pub window_menu: &'static str,
    pub help_menu: &'static str,
    pub new_file: &'static str,
    pub open_file: &'static str,
    pub save_file: &'static str,
    pub close_tab: &'static str,
    pub project_tree: &'static str,
    pub inspector: &'static str,
    pub search: &'static str,
    pub problems: &'static str,
    pub output: &'static str,
    pub canvas_preview: &'static str,
    pub settings: &'static str,
    pub command_palette: &'static str,
    pub ready: &'static str,
    pub line: &'static str,
    pub column: &'static str,
    pub word_wrap: &'static str,
    pub tab_size: &'static str,
    pub theme: &'static str,
    pub language: &'static str,
    pub font_size: &'static str,
    pub auto_save: &'static str,
    pub reset_defaults: &'static str,
    pub filter_files: &'static str,
    pub search_placeholder: &'static str,
    pub no_results: &'static str,
}

impl StudioLocale {
    #[must_use]
    pub fn strings(&self) -> StudioStrings {
        match self {
            Self::English => StudioStrings {
                app_title: "Incular Studio",
                file_menu: "File",
                edit_menu: "Edit",
                view_menu: "View",
                window_menu: "Window",
                help_menu: "Help",
                new_file: "New File",
                open_file: "Open...",
                save_file: "Save",
                close_tab: "Close Tab",
                project_tree: "Explorer",
                inspector: "Inspector",
                search: "Search",
                problems: "Problems",
                output: "Output",
                canvas_preview: "Canvas",
                settings: "Settings",
                command_palette: "Command Palette",
                ready: "Ready",
                line: "Ln",
                column: "Col",
                word_wrap: "Word Wrap",
                tab_size: "Tab Size",
                theme: "Color Theme",
                language: "Display Language",
                font_size: "Editor Font Size",
                auto_save: "Auto Save",
                reset_defaults: "Reset to Defaults",
                filter_files: "Filter files (Ctrl+P)...",
                search_placeholder: "Search in workspace (Ctrl+Shift+F)...",
                no_results: "No matches found.",
            },
            Self::Arabic => StudioStrings {
                app_title: "إنكيولار ستوديو",
                file_menu: "ملف",
                edit_menu: "تعديل",
                view_menu: "عرض",
                window_menu: "نافذة",
                help_menu: "مساعدة",
                new_file: "ملف جديد",
                open_file: "فتح...",
                save_file: "حفظ",
                close_tab: "إغلاق التبويب",
                project_tree: "المستعرض",
                inspector: "الفاحص",
                search: "بحث",
                problems: "المشاكل",
                output: "المخرجات",
                canvas_preview: "لوحة الرسم",
                settings: "الإعدادات",
                command_palette: "لوحة الأوامر",
                ready: "جاهز",
                line: "السطر",
                column: "العمود",
                word_wrap: "التفاف النص",
                tab_size: "حجم الجدولة",
                theme: "المظهر",
                language: "اللغة",
                font_size: "حجم الخط",
                auto_save: "حفظ تلقائي",
                reset_defaults: "استعادة الافتراضي",
                filter_files: "تصفية الملفات...",
                search_placeholder: "بحث في مساحة العمل...",
                no_results: "لا توجد نتائج مطابقة.",
            },
            Self::Hindi => StudioStrings {
                app_title: "इनक्युलर स्टूडियो",
                file_menu: "फ़ाइल",
                edit_menu: "संपादित करें",
                view_menu: "देखें",
                window_menu: "विंडो",
                help_menu: "सहायता",
                new_file: "नई फ़ाइल",
                open_file: "खोलें...",
                save_file: "सहेजें",
                close_tab: "टैब बंद करें",
                project_tree: "एक्सप्लोरर",
                inspector: "निरीक्षक",
                search: "खोजें",
                problems: "समस्याएं",
                output: "आउटपुट",
                canvas_preview: "कैनवस",
                settings: "सेटिंग्स",
                command_palette: "कमांड पैलेट",
                ready: "तैयार",
                line: "पंक्ति",
                column: "स्तंभ",
                word_wrap: "वर्ड रैप",
                tab_size: "टैब आकार",
                theme: "रंग थीम",
                language: "भाषा",
                font_size: "फॉन्ट आकार",
                auto_save: "स्वचालित सहेजें",
                reset_defaults: "डिफ़ॉल्ट रीसेट करें",
                filter_files: "फ़ाइलें फ़िल्टर करें...",
                search_placeholder: "कार्यस्थान में खोजें...",
                no_results: "कोई परिणाम नहीं मिला।",
            },
            Self::Japanese => StudioStrings {
                app_title: "Incular Studio",
                file_menu: "ファイル",
                edit_menu: "編集",
                view_menu: "表示",
                window_menu: "ウィンドウ",
                help_menu: "ヘルプ",
                new_file: "新規ファイル",
                open_file: "開く...",
                save_file: "保存",
                close_tab: "タブを閉じる",
                project_tree: "エクスプローラー",
                inspector: "インスペクター",
                search: "検索",
                problems: "問題",
                output: "出力",
                canvas_preview: "キャンバス",
                settings: "設定",
                command_palette: "コマンドパレット",
                ready: "準備完了",
                line: "行",
                column: "列",
                word_wrap: "折り返し",
                tab_size: "タブサイズ",
                theme: "カラーテーマ",
                language: "表示言語",
                font_size: "フォントサイズ",
                auto_save: "自動保存",
                reset_defaults: "初期設定に戻す",
                filter_files: "ファイルを絞り込み...",
                search_placeholder: "ワークスペース内を検索...",
                no_results: "一致する項目はありません。",
            },
        }
    }
}
