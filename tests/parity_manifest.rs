//! Contract test for the checked-in Flutter-derived parity inventory.
//!
//! The manifest uses one compact JSON object per line. It deliberately avoids
//! a parser dependency in the workspace test crate while remaining consumable
//! by normal JSON-lines tooling.

use std::{collections::HashSet, fs, path::Path};

const REQUIRED_FIELDS: [&str; 7] = [
    "flutter",
    "equivalent",
    "status",
    "owner",
    "public_path",
    "evidence",
    "notes",
];
const STATES: [&str; 5] = ["IMPLEMENTED", "MERGED", "INTERNAL", "DEFERRED", "SKIPPED"];

#[test]
fn widget_parity_manifest_is_complete_and_has_only_deliberate_gaps() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/widget_parity.jsonl");
    let source = fs::read_to_string(&path).expect("widget parity manifest must be checked in");
    let mut capabilities = HashSet::new();
    let mut totals = [0_usize; 5];

    for (line_number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.starts_with('{') && line.ends_with('}'),
            "manifest line {} is not a JSON object",
            line_number + 1
        );
        let mut values = Vec::new();
        for field in REQUIRED_FIELDS {
            let value = json_string_field(line, field).unwrap_or_else(|| {
                panic!("manifest line {} is missing {field:?}", line_number + 1)
            });
            assert!(
                !value.trim().is_empty(),
                "manifest line {} has an empty {field:?}",
                line_number + 1
            );
            assert!(
                !value.contains("TODO") && !value.contains("todo"),
                "manifest line {} uses an unresolved TODO in {field:?}",
                line_number + 1
            );
            values.push(value);
        }
        let capability = values[0];
        assert!(
            capabilities.insert(capability),
            "duplicate Flutter capability {capability:?} in manifest"
        );
        let status = values[2];
        let index = STATES
            .iter()
            .position(|known| *known == status)
            .unwrap_or_else(|| {
                panic!(
                    "manifest line {} has unresolved or unknown status {status:?}",
                    line_number + 1
                )
            });
        totals[index] += 1;
        if matches!(status, "DEFERRED" | "SKIPPED") {
            assert!(
                values[6].len() > 12,
                "manifest line {} needs a specific rationale for {status}",
                line_number + 1
            );
        }
        if status == "DEFERRED" {
            assert!(
                values[6].to_ascii_lowercase().contains("prerequisite"),
                "manifest line {} must name the prerequisite for DEFERRED work",
                line_number + 1
            );
        }
        if matches!(status, "IMPLEMENTED" | "MERGED") {
            assert!(
                values[5].contains("test") || values[5].contains("examples/"),
                "manifest line {} needs behavioral evidence for {status}",
                line_number + 1
            );
        }
    }

    assert!(
        capabilities.len() >= 90,
        "inventory was unexpectedly shortened"
    );
    eprintln!(
        "widget parity: implemented: {}, merged: {}, internal: {}, deferred: {}, skipped: {}, unresolved: 0",
        totals[0], totals[1], totals[2], totals[3], totals[4]
    );
}

const FLUTTER_API_REQUIRED_FIELDS: [&str; 12] = [
    "flutter",
    "incular",
    "category",
    "status",
    "constructor_parity",
    "parameter_parity",
    "default_parity",
    "behavior_parity",
    "flutter_docs",
    "public_path",
    "evidence",
    "notes",
];

#[test]
fn flutter_api_parity_manifest_is_complete_and_has_only_deliberate_gaps() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/flutter_api_parity.jsonl");
    let source = fs::read_to_string(&path).expect("flutter API parity manifest must be checked in");
    let mut capabilities = HashSet::new();
    let mut totals = [0_usize; 5];

    for (line_number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.starts_with('{') && line.ends_with('}'),
            "manifest line {} is not a JSON object",
            line_number + 1
        );
        let mut values = Vec::new();
        for field in FLUTTER_API_REQUIRED_FIELDS {
            let value = json_string_field(line, field).unwrap_or_else(|| {
                panic!("manifest line {} is missing {field:?}", line_number + 1)
            });
            assert!(
                !value.trim().is_empty(),
                "manifest line {} has an empty {field:?}",
                line_number + 1
            );
            assert!(
                !value.contains("TODO") && !value.contains("todo"),
                "manifest line {} uses an unresolved TODO in {field:?}",
                line_number + 1
            );
            values.push(value);
        }
        let capability = values[0];
        assert!(
            capabilities.insert(capability),
            "duplicate Flutter API {capability:?} in manifest"
        );
        let status = values[3];
        let index = STATES
            .iter()
            .position(|known| *known == status)
            .unwrap_or_else(|| {
                panic!(
                    "manifest line {} has unresolved or unknown status {status:?}",
                    line_number + 1
                )
            });
        totals[index] += 1;
        if matches!(status, "DEFERRED" | "SKIPPED") {
            assert!(
                values[11].len() > 12,
                "manifest line {} needs a specific rationale for {status}",
                line_number + 1
            );
        }
    }

    assert!(
        capabilities.len() >= 120,
        "Flutter API parity manifest was unexpectedly shortened"
    );
    eprintln!(
        "flutter API parity: implemented: {}, merged: {}, internal: {}, deferred: {}, skipped: {}, unresolved: 0",
        totals[0], totals[1], totals[2], totals[3], totals[4]
    );
}

#[test]
fn canonical_flutter_core_widgets_inventory_has_no_silently_omitted_names() {
    let canonical_flutter_widgets: &[&str] = &[
        // Core
        "Widget",
        "StatelessWidget / StatefulWidget",
        "BuildContext",
        "Key / ValueKey / UniqueKey",
        "GlobalKey",
        "Element / BuildOwner / BuildScope",
        "RenderObject / RenderBox / RenderSliver",
        "Layer subclasses",
        "InheritedWidget",
        "Notification",
        // Layout & Box Sizing
        "Row",
        "Column",
        "Flex",
        "Flexible",
        "Expanded",
        "Spacer",
        "Stack",
        "Positioned",
        "PositionedDirectional",
        "IndexedStack",
        "Wrap",
        "Table",
        "TableCell",
        "Container",
        "Padding",
        "Align",
        "Center",
        "SizedBox",
        "SizedOverflowBox",
        "ColoredBox",
        "ConstrainedBox",
        "UnconstrainedBox",
        "LimitedBox",
        "OverflowBox",
        "FractionallySizedBox",
        "FractionalTranslation",
        "AspectRatio",
        "FittedBox",
        "Baseline",
        "IgnoreBaseline",
        "IntrinsicWidth",
        "IntrinsicHeight",
        "ConstraintsTransformBox",
        "RotatedBox",
        "KeyedSubtree",
        "Placeholder",
        "PreferredSize",
        "OverflowBar",
        "NavigationToolbar",
        "InteractiveViewer",
        // Environment & Theming
        "Directionality",
        "DefaultTextStyle",
        "IconTheme",
        "OrientationBuilder",
        "ScrollConfiguration",
        "PrimaryScrollController",
        "TickerMode",
        "SensitiveContent",
        "SensitiveContentHost",
        "LookupBoundary",
        "SharedAppData",
        // Painting, Effects, Clipping & Masking
        "DecoratedBox",
        "CustomPaint",
        "Opacity",
        "ColorFiltered",
        "RawImage",
        "ImageIcon",
        "ImageFiltered",
        "ShaderMask",
        "BackdropFilter",
        "AnnotatedRegion",
        "SnapshotWidget",
        "ClipRect",
        "ClipRRect",
        "ClipOval",
        "ClipPath",
        "ClipRSuperellipse",
        "PhysicalModel",
        "PhysicalShape",
        "GridPaper",
        "Transform",
        "RepaintBoundary",
        // Animation Transitions
        "FadeTransition",
        "ScaleTransition",
        "RotationTransition",
        "SlideTransition",
        "AlignTransition",
        "SizeTransition",
        "PositionedTransition",
        "RelativePositionedTransition",
        "DecoratedBoxTransition",
        "DefaultTextStyleTransition",
        "MatrixTransition",
        "DualTransitionBuilder",
        // Implicit Animations
        "AnimatedContainer",
        "AnimatedAlign",
        "AnimatedPadding",
        "AnimatedOpacity",
        "AnimatedPositioned",
        "AnimatedPositionedDirectional",
        "AnimatedFractionallySizedBox",
        "AnimatedRotation",
        "AnimatedScale",
        "AnimatedSlide",
        "AnimatedSize",
        "AnimatedDefaultTextStyle",
        "AnimatedPhysicalModel",
        "AnimatedCrossFade",
        "AnimatedSwitcher",
        // Reactive Builders & State
        "AnimatedBuilder",
        "ListenableBuilder",
        "ValueListenableBuilder",
        "FutureBuilder",
        "StreamBuilder",
        "StatefulBuilder",
        "TweenAnimationBuilder",
        "RepeatingAnimationBuilder",
        "LayoutBuilder",
        // Hero
        "Hero",
        "HeroMode",
        "HeroControllerScope",
        // Scrolling & Viewports
        "SingleChildScrollView",
        "Scrollable",
        "NestedScrollView",
        "Viewport",
        "ShrinkWrappingViewport",
        "RawScrollbar",
        "ListBody",
        "ListWheelScrollView",
        "ListWheelViewport",
        "DraggableScrollableSheet",
        "DraggableScrollableActuator",
        "NotificationListener",
        "ScrollNotificationObserver",
        "TwoDimensionalScrollable",
        "TwoDimensionalScrollView",
        "TwoDimensionalViewport",
        "ListView",
        "GridView",
        "PageView",
        "CustomScrollView",
        // Slivers
        "SliverList",
        "SliverGrid",
        "SliverPadding",
        "SliverAppBar",
        "SliverPersistentHeader",
        "SliverToBoxAdapter",
        "SliverFixedExtentList",
        "SliverVariedExtentList",
        "SliverPrototypeExtentList",
        "SliverFillRemaining",
        "SliverFillViewport",
        "SliverLayoutBuilder",
        "SliverMainAxisGroup",
        "SliverCrossAxisGroup",
        "SliverCrossAxisExpanded",
        "SliverConstrainedCrossAxis",
        "DecoratedSliver",
        "SliverOpacity",
        "SliverAnimatedOpacity",
        "SliverFadeTransition",
        "SliverOffstage",
        "SliverIgnorePointer",
        "SliverSafeArea",
        "SliverVisibility",
        "PinnedHeaderSliver",
        "SliverFloatingHeader",
        "SliverResizingHeader",
        "SliverOverlapAbsorber",
        "SliverOverlapInjector",
        "SliverReorderableList",
        "TreeSliver",
        "AnimatedList",
        "AnimatedGrid",
        "SliverAnimatedList",
        "SliverAnimatedGrid",
        // Gestures & Drag-Drop
        "GestureDetector",
        "Listener",
        "RawGestureDetector",
        "MouseRegion",
        "IgnorePointer",
        "AbsorbPointer",
        "TapRegion",
        "TapRegionSurface",
        "TextFieldTapRegion",
        "Draggable",
        "LongPressDraggable",
        "DragTarget",
        "Dismissible",
        "ReorderableList",
        "ReorderableDragStartListener",
        "ReorderableDelayedDragStartListener",
        // Focus & Keyboard
        "Focus",
        "FocusScope",
        "KeyboardListener",
        "Shortcuts",
        "Actions",
        "ActionListener",
        "CallbackShortcuts",
        "FocusableActionDetector",
        "FocusTraversalGroup",
        "FocusTraversalOrder",
        "ExcludeFocus",
        "ExcludeFocusTraversal",
        "ShortcutRegistrar",
        // Selection & Radio
        "SelectionArea",
        "SelectableText",
        "SelectableRegion",
        "SelectionContainer",
        "SelectionListener",
        "RadioGroup",
        "RawRadio",
        // Forms
        "Form",
        "TextFormField",
        "FormField",
        "TextField",
        "TextArea",
        "Autocomplete",
        "RawAutocomplete",
        "AutocompleteHighlightedOption",
        "AutofillGroup",
        "UndoHistory",
        // Navigation & Multi-View
        "Navigator",
        "Router",
        "PopScope",
        "NavigatorPopHandler",
        "BackButtonListener",
        "Overlay",
        "OverlayEntry",
        "OverlayPortal",
        "AnimatedModalBarrier",
        "PageStorage",
        "RootRestorationScope",
        "UnmanagedRestorationScope",
        "WidgetsApp",
        "View",
        "ViewAnchor",
        "Title",
        // Platform & Menus
        "PlatformMenuBar",
        "CompositedTransformTarget",
        "CompositedTransformFollower",
        "RawTooltip",
        "RawMenuAnchor",
        "MenuBar",
        "MenuAnchor",
        // Utility & Accessibility
        "Expansible",
        "ErrorWidget",
        "Banner",
        "CheckedModeBanner",
        "PerformanceOverlay",
        "KeepAlive",
        "AutomaticKeepAlive",
        "IndexedSemantics",
        "SemanticsDebugger",
        "Semantics",
        "SafeArea",
        "Visibility",
        "Offstage",
        "Text",
        "Icon",
        "Image",
        "Button",
    ];

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs/flutter_api_parity.jsonl");
    let source = fs::read_to_string(&path).expect("flutter API parity manifest must be checked in");

    let manifest_names: HashSet<&str> = source
        .lines()
        .filter_map(|line| json_string_field(line.trim(), "flutter"))
        .collect();

    for widget in canonical_flutter_widgets {
        assert!(
            manifest_names.contains(widget),
            "canonical Flutter widget {widget:?} is missing from flutter_api_parity.jsonl"
        );
    }
}

fn json_string_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("\"{field}\":\"");
    let start = line.find(&prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}
