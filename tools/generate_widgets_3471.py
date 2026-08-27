#!/usr/bin/env python3
"""Generate the checked-in Flutter 3.47.1 widgets.dart export graph.

The Flutter checkout is intentionally kept outside the repository.  Pass its
checkout directory with ``--flutter-root`` or set ``INCULAR_FLUTTER_3471``
(``FLUTTER_ROOT`` is accepted as a convenience).  The historical temporary
checkout remains the local fallback so existing developer environments keep
working, but regeneration is no longer tied to that path.  This is a small
lexical scanner (not a Dart semantic analyzer): declarations are collected
only at top level, with comments/strings removed before matching.
"""

from __future__ import annotations

import json
import re
import argparse
import os
from pathlib import Path


DEFAULT_FLUTTER_CHECKOUT = Path("/tmp/flutter-3471.HbskVh")
REPO_ROOT = Path(__file__).resolve().parents[1]
FLUTTER = Path(
    os.environ.get("INCULAR_FLUTTER_3471")
    or os.environ.get("FLUTTER_ROOT")
    or DEFAULT_FLUTTER_CHECKOUT
) / "packages/flutter/lib"
OUT = REPO_ROOT / "specs"
REPORT_DIR = REPO_ROOT / "target"
COMMIT = "6655482ec06e547f90abf8ae7590466f4415978d"
PREVIOUS_COMMIT = "4cf24164269a5ebf0c16a028a00727d0e77bbb05"


def strip_comments_and_strings(source: str) -> str:
    """Keep newlines while replacing comments and string contents with spaces."""

    out: list[str] = []
    i = 0
    state = "code"
    quote = ""
    while i < len(source):
        ch = source[i]
        nxt = source[i + 1] if i + 1 < len(source) else ""
        if state == "code":
            if ch == "/" and nxt == "/":
                out.extend("  ")
                i += 2
                state = "line"
                continue
            if ch == "/" and nxt == "*":
                out.extend("  ")
                i += 2
                state = "block"
                continue
            if ch in ("'", '"'):
                quote = ch
                out.append(" ")
                i += 1
                state = "string"
                continue
            out.append(ch)
            i += 1
            continue
        if state == "line":
            if ch == "\n":
                out.append("\n")
                state = "code"
            else:
                out.append(" ")
            i += 1
            continue
        if state == "block":
            if ch == "*" and nxt == "/":
                out.extend("  ")
                i += 2
                state = "code"
            else:
                out.append("\n" if ch == "\n" else " ")
                i += 1
            continue
        # Dart supports adjacent string literals and interpolation.  For the
        # export graph, treating the complete literal as opaque is sufficient.
        if ch == "\\":
            out.extend("  ")
            i += 2
        elif ch == quote:
            out.append(" ")
            i += 1
            state = "code"
        else:
            out.append("\n" if ch == "\n" else " ")
            i += 1
    return "".join(out)


def brace_depths(source: str) -> list[int]:
    depth = 0
    depths: list[int] = []
    for ch in source:
        depths.append(depth)
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth = max(0, depth - 1)
    return depths


DECL_RE = re.compile(
    r"(?m)^[ \t]*(?:(?:abstract|base|interface|final|sealed|augment|mixin)\s+)*"
    r"(class|mixin|enum|typedef|extension(?:\s+type)?)\s+"
    r"([A-Za-z_$][\w$]*)"
)
TOP_VAR_RE = re.compile(
    r"^[ \t]*(?:const|final|late\s+final|late|var)\s+"
    r"(?:[A-Za-z_$][\w$<>?,.\[\] ]*\s+)?([A-Za-z_$][\w$]*)\s*(?:=|;|,|\()"
)
TOP_FUNC_RE = re.compile(
    r"^[ \t]*(?:external\s+)?(?:static\s+)?"
    r"(?:[A-Za-z_$][\w$<>?,.\[\].]*\s+)+([a-z_$][\w$]*)\s*\("
)


def declarations(path: str, shown: list[str] | None) -> list[tuple[str, str]]:
    source = (FLUTTER / path).read_text(encoding="utf-8")
    clean = strip_comments_and_strings(source)
    depths = brace_depths(clean)
    found: list[tuple[str, str]] = []

    for match in DECL_RE.finditer(clean):
        if depths[match.start()] != 0:
            continue
        name = match.group(2)
        if name.startswith("_"):
            continue
        found.append((name, match.group(1).upper()))

    # Matching line-by-line avoids a pathological backtracking case in very
    # large framework source files.  Public Dart declarations begin at the
    # top-level line in the Flutter sources; continuation lines are not
    # needed to identify their names.
    offset = 0
    for line in clean.splitlines(keepends=True):
        if offset < len(depths) and depths[offset] == 0:
            match = TOP_VAR_RE.match(line)
            if match:
                name = match.group(1)
                if not name.startswith("_"):
                    found.append((name, "CONSTANT"))
            match = TOP_FUNC_RE.match(line)
            if match:
                name = match.group(1)
                if not name.startswith("_") and name not in {"if", "for", "while", "switch", "class"}:
                    found.append((name, "FUNCTION"))
        offset += len(line)

    if shown:
        allowed = set(shown)
        found = [(name, kind) for name, kind in found if name in allowed]

    result: list[tuple[str, str]] = []
    seen: set[str] = set()
    for item in found:
        if item[0] not in seen:
            result.append(item)
            seen.add(item[0])
    return result


EXPORT_RE = re.compile(r"^export '([^']+)'(?:\s+(show|hide)\s+([^;]+))?;", re.M)


def export_directives(path: str) -> list[dict[str, object]]:
    source = (FLUTTER / path).read_text(encoding="utf-8")
    result: list[dict[str, object]] = []
    for match in EXPORT_RE.finditer(source):
        value = match.group(1)
        mode = match.group(2)
        selected = [part.strip() for part in match.group(3).split(",")] if match.group(3) else None
        result.append(
            {
                "directive": "export",
                "uri": value,
                "mode": mode or "all",
                "show": selected if mode == "show" else None,
                "hide": selected if mode == "hide" else None,
                "origin": "external" if value.startswith("package:") else value,
            }
        )
    return result


def direct_exports() -> list[dict[str, object]]:
    return export_directives("widgets.dart")


def resolve_uri(parent: str, uri: str) -> str | None:
    if uri.startswith("dart:"):
        return None
    if uri.startswith("package:flutter/"):
        return uri.removeprefix("package:flutter/")
    if uri.startswith("package:"):
        return None
    return str((Path(parent).parent / uri).as_posix())


_DECLARATION_CACHE: dict[str, list[tuple[str, str, str]]] = {}


def set_flutter_root(value: str | Path) -> None:
    """Point the scanner at a Flutter checkout or its ``packages/flutter/lib``."""

    global FLUTTER
    root = Path(value).expanduser()
    if (root / "widgets.dart").is_file():
        FLUTTER = root
    else:
        FLUTTER = root / "packages/flutter/lib"
    if not (FLUTTER / "widgets.dart").is_file():
        raise SystemExit(
            "Flutter 3.47.1 checkout not found: expected "
            f"{FLUTTER / 'widgets.dart'}; pass --flutter-root or set "
            "INCULAR_FLUTTER_3471"
        )
    _DECLARATION_CACHE.clear()


def recursive_declarations(path: str, stack: set[str] | None = None) -> list[tuple[str, str, str]]:
    """Return (symbol, kind, declaring source) from a Flutter export graph."""

    stack = set() if stack is None else stack
    if path in _DECLARATION_CACHE:
        return _DECLARATION_CACHE[path]
    if path in stack:
        return []
    stack.add(path)
    source = (FLUTTER / path).read_text(encoding="utf-8")
    values = [(name, kind, path) for name, kind in declarations(path, None)]
    for directive in export_directives(path):
        uri = str(directive["uri"])
        child = resolve_uri(path, uri)
        if child is None:
            # The Flutter checkout does not contain SDK libraries such as
            # dart:ui. Their public names still enter widgets.dart through
            # explicit `show` exports (painting, services, semantics, ...),
            # so retain those names with an external source marker instead of
            # silently dropping them from the canonical graph.
            if uri == "dart:ui" and directive["show"]:
                for name in directive["show"]:
                    values.append((str(name), external_symbol_kind(str(name)), uri))
            continue
        nested = recursive_declarations(child, stack)
        shown = set(directive["show"] or [])
        hidden = set(directive["hide"] or [])
        if shown:
            nested = [row for row in nested if row[0] in shown]
        elif hidden:
            nested = [row for row in nested if row[0] not in hidden]
        values.extend(nested)
    stack.remove(path)
    _DECLARATION_CACHE[path] = values
    return values


def external_symbol_kind(name: str) -> str:
    """Best-effort kind for declarations supplied by SDK libraries."""

    if name.startswith("k"):
        return "CONSTANT"
    if name.endswith("Callback") or name.endswith("Listener"):
        return "TYPEDEF"
    if name and name[0].islower():
        return "FUNCTION"
    return "CLASS"


def is_deprecated(path: str, name: str) -> bool:
    """Detect a declaration-level ``@Deprecated`` annotation.

    Member annotations are intentionally ignored: parity rows represent
    exported symbols, not every member. The bounded look-ahead prevents a
    deprecated annotation on one declaration from spilling into the next one.
    """

    if path.startswith("dart:") or path.startswith("package:"):
        return False
    clean = strip_comments_and_strings((FLUTTER / path).read_text(encoding="utf-8"))
    for annotation in re.finditer(r"@Deprecated\s*\([^)]*\)", clean, re.S):
        tail = clean[annotation.end() : annotation.end() + 700]
        declaration = re.search(
            rf"\b(?:abstract\s+|base\s+|interface\s+|final\s+|sealed\s+|augment\s+|mixin\s+)*"
            rf"(?:class|mixin|enum|typedef|extension(?:\s+type)?)\s+{re.escape(name)}\b",
            tail,
        )
        if declaration:
            return True
    return False


def rust_symbols() -> set[str]:
    """Return symbols actually exposed by the curated Widgets crate root.

    Scanning every Rust source file is deliberately forbidden here: retained
    implementation types are public inside private modules so sibling crates
    can use the internal bridge, but that does not make them application API.
    The parity graph must distinguish those names from root ``pub use`` items.
    """

    source = (REPO_ROOT / "crates/incular-widgets/src/lib.rs").read_text(encoding="utf-8")
    names: set[str] = set()
    # Capture each root re-export statement, including multiline groups.  The
    # root has no public declarations other than the compatibility IntoWidget
    # trait and children! macro, neither of which is a Flutter graph symbol.
    for statement in re.findall(r"\bpub\s+use\s+([^;]+);", source, re.S):
        if "{" in statement:
            body = statement.rsplit("{", 1)[1].rsplit("}", 1)[0]
            candidates = body.split(",")
        else:
            candidates = [statement.rsplit("::", 1)[-1]]
        for candidate in candidates:
            name = candidate.strip().split(" as ", 1)[0].strip()
            if re.fullmatch(r"[A-Za-z_]\w*", name) and name != "self":
                names.add(name)
    return names


DOMAIN_OWNERS = {
    "AnimationController": "incular-animation",
    "AnimationStatus": "incular-animation",
    "Animatable": "incular-animation",
    "Tween": "incular-animation",
    "Curve": "incular-animation",
    "Curves": "incular-animation",
    "Color": "incular-core",
    "Brightness": "incular-config",
    "UniqueKey": "incular-core",
    "TextSelectionHandleType": "incular-rendering",
    "TextStyle": "incular-text",
    "TextSpan": "incular-text",
    "WidgetSpan": "incular-text",
    "RichText": "incular-text",
    "TextAlign": "incular-text",
    "TextOverflow": "incular-text",
    "TextScaler": "incular-text",
    "TextHeightBehavior": "incular-text",
    "TextWidthBasis": "incular-text",
    "StrutStyle": "incular-text",
    "ScrollPhysics": "incular-scroll",
    "ScrollController": "incular-scroll",
    "ScrollMetrics": "incular-scroll",
    "ScrollPosition": "incular-scroll",
    "ScrollNotification": "incular-scroll",
    "Offset": "incular-core",
    "Size": "incular-core",
    "Rect": "incular-core",
    "EdgeInsets": "incular-config",
    "Axis": "incular-config",
    "TextDirection": "incular-config",
    "Matrix4": "incular-rendering",
    "Characters": "characters",
    "Key": "incular-core",
    "LocalKey": "incular-core",
    "TextEditingController": "incular-text",
    "TextEditingValue": "incular-text",
    "TextRange": "incular-text",
    "TextSelection": "incular-text",
    "AssetImage": "incular-image",
    "FileImage": "incular-image",
    "ImageProvider": "incular-image",
    "MemoryImage": "incular-image",
}

# A Widgets export is often a domain value re-exported from another Flutter
# library. Keep the physical Rust owner authoritative even when the Widgets
# facade also exposes the name. This table is intentionally explicit so a new
# domain type cannot silently become a second implementation in
# `incular-widgets` merely because a glob or helper happens to mention it.
DOMAIN_OWNERS.update(
    {
        name: "incular-config"
        for name in {
            "Alignment",
            "AlignmentDirectional",
            "AlignmentGeometry",
            "Axis",
            "AxisDirection",
            "BoxConstraints",
            "Clip",
            "Constraints",
            "CrossAxisAlignment",
            "EdgeInsets",
            "EdgeInsetsDirectional",
            "FlexFit",
            "FractionalOffset",
            "MainAxisAlignment",
            "MainAxisSize",
            "StackFit",
            "TextDirection",
            "VerticalDirection",
            "WrapAlignment",
            "WrapCrossAlignment",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-animation"
        for name in {
            "Animation",
            "AnimationStatus",
            "Animatable",
            "Curve",
            "Curves",
            "Tween",
            "TweenSequence",
            "TweenSequenceItem",
            "CurvedAnimation",
            "AnimationController",
            "Simulation",
            "Tolerance",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-text"
        for name in {
            "Characters",
            "TextAlign",
            "TextAlignVertical",
            "TextBaseline",
            "TextDecoration",
            "TextDecorationStyle",
            "TextHeightBehavior",
            "TextLeadingDistribution",
            "TextOverflow",
            "TextPosition",
            "TextRange",
            "TextScaler",
            "TextSelection",
            "TextShadow",
            "TextSpan",
            "TextStyle",
            "TextWidthBasis",
            "WidgetSpan",
            "RichText",
            "StrutStyle",
            "InlineSpan",
            "PlaceholderSpan",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-scroll"
        for name in {
            "AlwaysScrollableScrollPhysics",
            "BouncingScrollPhysics",
            "ClampingScrollPhysics",
            "FixedExtentScrollPhysics",
            "NeverScrollableScrollPhysics",
            "PageScrollPhysics",
            "RangeMaintainingScrollPhysics",
            "ScrollBehavior",
            "ScrollController",
            "ScrollMetrics",
            "ScrollNotification",
            "ScrollPhysics",
            "ScrollPosition",
            "TrackingScrollController",
            "ViewportOffset",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-core"
        for name in {
            "Color",
            "ColorSwatch",
            "Offset",
            "Rect",
            "Size",
            "UniqueKey",
            "ValueKey",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-gestures"
        for name in {
            "DragDownDetails",
            "DragEndDetails",
            "DragStartDetails",
            "DragUpdateDetails",
            "LongPressEndDetails",
            "LongPressMoveUpdateDetails",
            "LongPressStartDetails",
            "PointerEvent",
            "ScaleEndDetails",
            "ScaleStartDetails",
            "ScaleUpdateDetails",
            "TapDownDetails",
            "TapUpDetails",
            "Velocity",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-navigation"
        for name in {
            "Navigator",
            "NavigatorObserver",
            "Overlay",
            "OverlayEntry",
            "Page",
            "Route",
            "RouteSettings",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-semantics"
        for name in {"SemanticsAction", "SemanticsFlag", "SemanticsHintOverrides", "SemanticsProperties"}
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-rendering"
        for name in {
            "Canvas",
            "ColorFilter",
            "ImageFilter",
            "Paint",
            "Path",
            "Shader",
            "Shadow",
            "PlaceholderAlignment",
            "LineMetrics",
        }
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-config"
        for name in {"Locale", "TargetPlatform"}
    }
)
DOMAIN_OWNERS.update(
    {
        name: "incular-text"
        for name in {"TextAffinity", "TextPosition"}
    }
)

PLATFORM_FILES = {
    "platform_view.dart",
    "platform_menu_bar.dart",
    "platform_selectable_region_context_menu.dart",
    "texture.dart",
    "view.dart",
    "app_lifecycle_listener.dart",
    "desktop_text_selection_toolbar_layout_delegate.dart",
}
RENDERER_FILES = {
    "basic.dart",
    "color_filter.dart",
    "image_filter.dart",
    "image.dart",
    "image_icon.dart",
    "snapshot_widget.dart",
    "two_dimensional_viewport.dart",
    "sliver_tree.dart",
    "stretch_effect.dart",
}
DART_MECHANIC_FILES = {
    "binding.dart",
    "framework.dart",
    "service_extensions.dart",
    "widget_inspector.dart",
    "slotted_render_object_widget.dart",
}


def public_path(name: str, owner: str, existing: set[str]) -> str | None:
    if owner == "characters":
        return "incular::text::Characters"
    if owner != "incular-widgets":
        return f"incular::{owner.removeprefix('incular-')}::{name}"
    if name in existing:
        return f"incular::widgets::{name}"
    return None


DOMAIN_ROOT_FILES = {
    "incular-animation": REPO_ROOT / "crates/incular-animation/src/lib.rs",
    "incular-config": REPO_ROOT / "crates/incular-config/src/lib.rs",
    "incular-core": REPO_ROOT / "crates/incular-core/src/lib.rs",
    "incular-gestures": REPO_ROOT / "crates/incular-gestures/src/lib.rs",
    "incular-navigation": REPO_ROOT / "crates/incular-navigation/src/lib.rs",
    "incular-rendering": REPO_ROOT / "crates/incular-rendering/src/lib.rs",
    "incular-scroll": REPO_ROOT / "crates/incular-scroll/src/lib.rs",
    "incular-semantics": REPO_ROOT / "crates/incular-semantics/src/lib.rs",
    "incular-text": REPO_ROOT / "crates/incular-text/src/lib.rs",
    "incular-image": REPO_ROOT / "crates/incular-image/src/lib.rs",
}


def domain_root_symbols() -> dict[str, set[str]]:
    """Read the public root of each authoritative domain crate.

    The graph must not call a symbol ``MERGED`` when the claimed owner only
    appears in a hand-written ownership table.  This lightweight scan is
    deliberately limited to crate roots; private implementation declarations
    are not application API.
    """

    result: dict[str, set[str]] = {}
    declaration_re = re.compile(
        r"\bpub\s+(?:(?:const|static)\s+)?"
        r"(?:struct|enum|type|trait|fn|const|static)\s+([A-Za-z_]\w*)"
    )
    for owner, path in DOMAIN_ROOT_FILES.items():
        if not path.is_file():
            result[owner] = set()
            continue
        source = path.read_text(encoding="utf-8")
        names = set(declaration_re.findall(source))
        for statement in re.findall(r"\bpub\s+use\s+([^;]+);", source, re.S):
            if "{" in statement:
                body = statement.rsplit("{", 1)[1].rsplit("}", 1)[0]
                candidates = body.split(",")
            else:
                candidates = [statement.rsplit("::", 1)[-1]]
            for candidate in candidates:
                name = candidate.strip().split(" as ", 1)[-1].strip()
                if re.fullmatch(r"[A-Za-z_]\w*", name) and name != "self":
                    names.add(name)
        result[owner] = names
    return result


def classify(
    name: str,
    source: str,
    kind: str,
    existing: set[str],
    domain_symbols: dict[str, set[str]],
    deprecated: bool,
) -> tuple[str, str, str]:
    if deprecated:
        return "SKIPPED_DEPRECATED", "incular-widgets", None
    if name in TASK24_CLOSURE:
        status, owner, mapped, _ = TASK24_CLOSURE[name]
        return status, owner, mapped
    if name in DOMAIN_OWNERS and (
        DOMAIN_OWNERS[name] == "characters"
        or name in domain_symbols.get(DOMAIN_OWNERS[name], set())
    ):
        status = "MERGED"
        owner = DOMAIN_OWNERS[name]
    elif name in existing:
        status = "RUSTIFIED"
        owner = "incular-widgets"
    elif source.rsplit("/", 1)[-1] in PLATFORM_FILES:
        status = "DEFERRED_PLATFORM"
        owner = "incular-widgets"
    elif source.rsplit("/", 1)[-1] in DART_MECHANIC_FILES:
        status = "SKIPPED_DART_MECHANIC"
        owner = "incular-widgets"
    elif source.rsplit("/", 1)[-1] in RENDERER_FILES:
        status = "DEFERRED_RENDERER"
        owner = "incular-widgets"
    elif kind in {"CLASS", "MIXIN", "ENUM", "TYPEDEF", "EXTENSION TYPE"}:
        status = "DEFERRED_RENDERER"
        owner = "incular-widgets"
    else:
        status = "SKIPPED_DART_MECHANIC"
        owner = "incular-widgets"
    return status, owner, public_path(name, owner, existing)


def public_symbol_kind(name: str, raw_kind: str, source: str, status: str) -> str:
    """Normalize Dart syntax kinds to the parity taxonomy used by Task 22."""

    if status == "SKIPPED_DART_MECHANIC":
        return "FRAMEWORK_INTERNAL_API"
    if raw_kind == "ENUM":
        return "ENUM"
    if raw_kind in {"MIXIN", "EXTENSION", "EXTENSION TYPE"}:
        return "TRAIT/EXTENSION_POINT"
    if raw_kind == "TYPEDEF":
        callback_markers = (
            "Callback",
            "Builder",
            "Factory",
            "Getter",
            "Setter",
            "Predicate",
            "Changed",
            "Listener",
        )
        return "CALLBACK" if name.endswith(callback_markers) else "TRAIT/EXTENSION_POINT"
    if raw_kind == "CONSTANT":
        return "CONSTANT"
    if raw_kind == "FUNCTION":
        return "FUNCTION"
    if name.endswith("Controller"):
        return "CONTROLLER"
    if name.endswith("Delegate") or name.endswith("Policy"):
        return "DELEGATE"
    if source.startswith("src/widgets/"):
        return "WIDGET"
    return "VALUE_TYPE"


FULL_MEMBER_AUDIT_TYPES = {
    # These families have a checked-in, member-level manifest with executable
    # tests.  Keeping the allow-list explicit prevents a type from being
    # labelled complete merely because a similarly named Rust descriptor
    # exists.
    "TextStyle",
    "FontWeight",
    "FontFeature",
    "FontVariation",
    "TextDecoration",
    "TextDecorationStyle",
    "StrutStyle",
    "TextShadow",
    "TextBaseline",
    "TextLeadingDistribution",
    "TextWidthBasis",
    "TextHeightBehavior",
    "EdgeInsets",
    "EdgeInsetsDirectional",
    "Alignment",
    "AlignmentDirectional",
    "FractionalOffset",
    "BoxDecoration",
    "Border",
    "BorderDirectional",
    "BorderSide",
    "BorderStyle",
    "BorderRadius",
    "BorderRadiusDirectional",
    "BoxShadow",
    "BlurStyle",
    "BoxShape",
    "Radius",
    "DecorationImage",
    "CustomPainter",
    "CustomPaint",
    "ScrollPhysics",
    "ScrollMetrics",
    "Simulation",
    "SpringDescription",
    "SpringSimulation",
    "ScrollSpringSimulation",
    "FrictionSimulation",
    "BoundedFrictionSimulation",
    "GravitySimulation",
    "Tolerance",
    "Curves",
    "Curve",
    "Tween",
    "TapDownDetails",
    "TapUpDetails",
    "DragDownDetails",
    "DragStartDetails",
    "DragUpdateDetails",
    "DragEndDetails",
    "ScaleStartDetails",
    "ScaleUpdateDetails",
    "ScaleEndDetails",
    "LongPressStartDetails",
    "LongPressMoveUpdateDetails",
    "LongPressEndDetails",
    "Velocity",
}


# Task 23 deliberately separates *what a Flutter symbol means to an Incular
# application* from the implementation status recorded above.  In
# particular, a symbol can be `DEFERRED_RENDERER` today and still be a
# `RUSTIFIED` P0 capability that belongs in the next implementation batch.
# Keep these sets explicit and reviewable: using a name/source heuristic for
# priorities makes it too easy for a new Flutter export to silently become a
# P0 commitment.
PORT_POLICIES = {
    "DIRECT",
    "RUSTIFIED",
    "MERGED_SIGNAL",
    "MERGED_CONTEXT",
    "MERGED_TOKIO",
    "MERGED_DOMAIN",
    "INTERNAL",
    "SKIP_DART_MECHANIC",
    "SKIP_LOW_VALUE",
    "DEFER_PLATFORM",
    "DEFER_RENDERER",
}
PRIORITIES = {
    "P0_DEPLOYABLE",
    "P1_COMMON",
    "P2_ADVANCED",
    "P3_COMPATIBILITY",
    "NEVER_PORT",
}


SIGNAL_MERGED = {
    "ChangeNotifier",
    "Listenable",
    "ValueNotifier",
    "ValueListenable",
    "ListenableBuilder",
    "ValueListenableBuilder",
    "StatefulBuilder",
}

# These are Flutter's ambient/inheritance mechanics, not a request to expose
# an InheritedWidget class hierarchy.  The useful values remain represented
# by typed environment/context dependencies in Incular.
CONTEXT_MERGED = {
    "InheritedWidget",
    "InheritedModel",
    "InheritedModelElement",
    "InheritedNotifier",
    "InheritedTheme",
    "InheritedElement",
    "ProxyWidget",
    "ProxyElement",
    "MediaQuery",
    "MediaQueryData",
    "Localizations",
    "LocalizationsDelegate",
    "LocalizationsResolver",
    "WidgetsLocalizations",
    "DefaultSelectionStyle",
    "DefaultTextStyle",
}

CONTEXT_NEVER_PORT = {
    "InheritedWidget",
    "InheritedModel",
    "InheritedModelElement",
    "InheritedNotifier",
    "InheritedTheme",
    "InheritedElement",
    "ProxyWidget",
    "ProxyElement",
}

TOKIO_MERGED = {
    "FutureBuilder",
    "StreamBuilder",
    "StreamBuilderBase",
    "AsyncSnapshot",
    "ConnectionState",
    "AsyncWidgetBuilder",
}

# Runtime mechanisms retained internally or replaced by a declarative
# capability.  They must not turn into public application mixins.
INTERNAL_RUNTIME = {
    "TickerProvider",
    "TickerProviderStateMixin",
    "SingleTickerProviderStateMixin",
    "TickerModeData",
    "AutomaticKeepAliveClientMixin",
    "KeepAliveHandle",
    "KeepAliveNotification",
}

NEVER_PORT_DART_ARCHITECTURE = {
    # Stateful/StatelessWidget and Element families are Dart's object model;
    # Incular uses retained descriptors plus external controllers/signals.
    "State",
    "StateSetter",
    "StatefulElement",
    "StatefulWidget",
    "StatefulWidgetBuilder",
    "StatelessElement",
    "StatelessWidget",
    "Element",
    "ElementVisitor",
    "ComponentElement",
    "ConditionalElementVisitor",
    "NotifiableElementMixin",
    "BuildContext",
    "BuildOwner",
    "BuildScope",
    "ProxyElement",
    # RenderObjectWidget/Element inheritance is deliberately not Incular's
    # public composition model.
    "RenderObject",
    "RenderBox",
    "RenderSliver",
    "RenderObjectWidget",
    "SingleChildRenderObjectWidget",
    "MultiChildRenderObjectWidget",
    "ParentDataWidget",
    "ParentDataElement",
    "LeafRenderObjectElement",
    "LeafRenderObjectWidget",
    "RenderObjectElement",
    "SingleChildRenderObjectElement",
    "MultiChildRenderObjectElement",
    "RootElement",
    "RootElementMixin",
    "RootRenderObjectElement",
    "RenderObjectToWidgetAdapter",
    "RenderObjectToWidgetElement",
    # There is no global widget-tree lookup mechanism.
    "GlobalKey",
    "GlobalObjectKey",
    "LabeledGlobalKey",
    # Restoration uses Incular's retained restoration data/serde model.
    "RestorationMixin",
    "RestorableProperty",
    "RestorableBool",
    "RestorableBoolN",
    "RestorableChangeNotifier",
    "RestorableDateTime",
    "RestorableDateTimeN",
    "RestorableDouble",
    "RestorableDoubleN",
    "RestorableEnum",
    "RestorableEnumN",
    "RestorableInt",
    "RestorableIntN",
    "RestorableListenable",
    "RestorableNum",
    "RestorableNumN",
    "RestorableRouteBuilder",
    "RestorableRouteFuture",
    "RestorableString",
    "RestorableStringN",
    "RestorableTextEditingController",
    "RestorableValue",
    "RestorationBucket",
}

# The names below describe capabilities that are intentionally implemented
# with Rust closures/traits/composition.  They are not Dart callback aliases
# or inheritance-compatible types.
RUSTIFIED_CAPABILITIES = {
    "Action",
    "ActionDispatcher",
    "ActionListener",
    "Actions",
    "CallbackAction",
    "ContextAction",
    "Intent",
    "Shortcuts",
    "ShortcutManager",
    "ShortcutRegistry",
    "ShortcutRegistryEntry",
    "ShortcutActivator",
    "SingleActivator",
    "LogicalKeySet",
    "KeySet",
    "KeyboardListener",
    "CustomPainter",
    "CustomClipper",
    "SingleChildLayoutDelegate",
    "MultiChildLayoutDelegate",
    "SliverPersistentHeaderDelegate",
    "RouteFactory",
    "RoutePageBuilder",
    "RoutePredicate",
    "RouteTransitionsBuilder",
    "RoutePresentationCallback",
    "RouteCompletionCallback",
    "PageRouteBuilder",
    "RawDialogRoute",
    "ModalRoute",
    "OverlayRoute",
    "TransitionRoute",
    "PopupRoute",
    "PageRoute",
    "Notification",
    "NotificationListener",
    "NotificationListenerCallback",
    "ScrollBehavior",
    "ScrollPhysics",
    "AlwaysScrollableScrollPhysics",
    "BouncingScrollPhysics",
    "ClampingScrollPhysics",
    "NeverScrollableScrollPhysics",
    "PageScrollPhysics",
    "RangeMaintainingScrollPhysics",
    "FilterQuality",
    "FocusScopeNode",
    "FontFeature",
    "FontVariation",
    "FontWeight",
    "FormState",
    "IconData",
    "ImageConfiguration",
    "PinnedHeaderSliver",
    "RestorationScope",
    "RotationTransition",
    "UndoHistoryController",
}

# Task 24 is a capability-closure audit, not a request to mint 35 Flutter
# compatibility classes.  Keep the decisions explicit and independent from
# the lexical source-file heuristic below.  The generated status column keeps
# the canonical `MERGED`/`RUSTIFIED` vocabulary used by the older inventory;
# `p0_closure_status` records the deployability decision required by Task 24.
TASK24_CLOSURE: dict[str, tuple[str, str, str, str]] = {
    # name: (canonical status, owner crate, public path, closure status)
    "Action": ("RUSTIFIED", "incular-gestures", "incular::gestures::Action", "RUSTIFIED_IMPLEMENTED"),
    "Actions": ("RUSTIFIED", "incular-gestures", "incular::gestures::Actions", "RUSTIFIED_IMPLEMENTED"),
    "FocusScopeNode": ("RUSTIFIED", "incular-gestures", "incular::gestures::FocusScopeNode", "RUSTIFIED_IMPLEMENTED"),
    "Intent": ("RUSTIFIED", "incular-gestures", "incular::gestures::Intent", "RUSTIFIED_IMPLEMENTED"),
    "Shortcuts": ("RUSTIFIED", "incular-gestures", "incular::gestures::Shortcuts", "RUSTIFIED_IMPLEMENTED"),
    "KeyboardListener": ("RUSTIFIED", "incular-widgets", "incular::widgets::KeyboardListener", "DIRECT_IMPLEMENTED"),
    "AlwaysScrollableScrollPhysics": ("MERGED", "incular-scroll", "incular::scroll::ScrollPhysics", "MERGED_IMPLEMENTED"),
    "BouncingScrollPhysics": ("MERGED", "incular-scroll", "incular::scroll::ScrollPhysics", "MERGED_IMPLEMENTED"),
    "NeverScrollableScrollPhysics": ("MERGED", "incular-scroll", "incular::scroll::ScrollPhysics", "MERGED_IMPLEMENTED"),
    "PageScrollPhysics": ("MERGED", "incular-scroll", "incular::scroll::ScrollPhysics", "MERGED_IMPLEMENTED"),
    "RangeMaintainingScrollPhysics": ("MERGED", "incular-scroll", "incular::scroll::ScrollPhysics", "MERGED_IMPLEMENTED"),
    "FilterQuality": ("MERGED", "incular-rendering", "incular::rendering::FilterQuality", "MERGED_IMPLEMENTED"),
    "FontFeature": ("MERGED", "incular-text", "incular::text::FontFeature", "MERGED_IMPLEMENTED"),
    "FontVariation": ("MERGED", "incular-text", "incular::text::FontVariation", "MERGED_IMPLEMENTED"),
    "FontWeight": ("MERGED", "incular-text", "incular::text::FontWeight", "MERGED_IMPLEMENTED"),
    "IconData": ("MERGED", "incular-text", "incular::text::IconData", "MERGED_IMPLEMENTED"),
    "ImageConfiguration": ("MERGED", "incular-image", "incular::image::ImageConfiguration", "MERGED_IMPLEMENTED"),
    "Paint": ("MERGED", "incular-rendering", "incular::rendering::Paint", "MERGED_IMPLEMENTED"),
    "Shader": ("MERGED", "incular-rendering", "incular::rendering::Shader", "MERGED_IMPLEMENTED"),
    "Shadow": ("MERGED", "incular-rendering", "incular::rendering::Shadow", "MERGED_IMPLEMENTED"),
    "FormState": ("RUSTIFIED", "incular-widgets", "incular::widgets::FormState", "RUSTIFIED_IMPLEMENTED"),
    "Localizations": ("MERGED", "incular-widgets", "incular::widgets::Localizations", "MERGED_IMPLEMENTED"),
    "MediaQueryData": ("MERGED", "incular-widgets", "incular::widgets::MediaQueryData", "MERGED_IMPLEMENTED"),
    "RestorationScope": ("MERGED", "incular-core", "incular::core::RestorationScope", "MERGED_IMPLEMENTED"),
    "UndoHistoryController": ("MERGED", "incular-runtime", "incular::runtime::UndoHistoryController", "MERGED_IMPLEMENTED"),
    "PinnedHeaderSliver": ("RUSTIFIED", "incular-widgets", "incular::widgets::PinnedHeaderSliver", "DIRECT_IMPLEMENTED"),
    "RotationTransition": ("RUSTIFIED", "incular-widgets", "incular::widgets::RotationTransition", "DIRECT_IMPLEMENTED"),
    "ModalRoute": ("RUSTIFIED", "incular-navigation", "incular::navigation::Route", "RUSTIFIED_IMPLEMENTED"),
    "OverlayRoute": ("RUSTIFIED", "incular-navigation", "incular::navigation::Route", "RUSTIFIED_IMPLEMENTED"),
    "PageRoute": ("RUSTIFIED", "incular-navigation", "incular::navigation::Route", "RUSTIFIED_IMPLEMENTED"),
    "PageRouteBuilder": ("RUSTIFIED", "incular-navigation", "incular::navigation::PageRouteBuilder", "RUSTIFIED_IMPLEMENTED"),
    "PopupRoute": ("RUSTIFIED", "incular-navigation", "incular::navigation::Route", "RUSTIFIED_IMPLEMENTED"),
    "RawDialogRoute": ("RUSTIFIED", "incular-navigation", "incular::navigation::Route", "RUSTIFIED_IMPLEMENTED"),
    "RouteSettings": ("MERGED", "incular-navigation", "incular::navigation::RouteSettings", "MERGED_IMPLEMENTED"),
    "TransitionRoute": ("RUSTIFIED", "incular-navigation", "incular::navigation::Route", "RUSTIFIED_IMPLEMENTED"),
}

TASK24_POLICIES = {
    name: "MERGED_CONTEXT"
    for name in {"Localizations", "MediaQueryData"}
}
TASK24_POLICIES.update(
    {
        name: "MERGED_DOMAIN"
        for name in {
            "AlwaysScrollableScrollPhysics", "BouncingScrollPhysics", "NeverScrollableScrollPhysics",
            "PageScrollPhysics", "RangeMaintainingScrollPhysics", "FilterQuality", "FontFeature",
            "FontVariation", "FontWeight", "IconData", "ImageConfiguration", "Paint", "Shader",
            "Shadow", "RestorationScope", "UndoHistoryController", "RouteSettings",
        }
    }
)
TASK24_POLICIES.update(
    {
        name: "RUSTIFIED"
        for name in {
            "Action", "Actions", "FocusScopeNode", "Intent", "Shortcuts", "KeyboardListener",
            "FormState", "ModalRoute", "OverlayRoute", "PageRoute", "PageRouteBuilder", "PopupRoute",
            "RawDialogRoute", "TransitionRoute", "PinnedHeaderSliver", "RotationTransition",
        }
    }
)

# These aliases are intentionally represented as ordinary Rust closures.  A
# named row is retained for source auditability, but no public alias is
# required by the Incular API.
CLOSURE_ALIASES = {
    "VoidCallback",
    "ValueChanged",
    "ValueGetter",
    "ValueSetter",
    "GestureTapCallback",
    "GestureDragUpdateCallback",
    "GestureDragDownCallback",
    "GestureDragEndCallback",
    "GestureDragStartCallback",
    "GestureLongPressCallback",
    "GestureScaleUpdateCallback",
    "GestureScaleStartCallback",
    "GestureScaleEndCallback",
    "RouteFactory",
    "WidgetBuilder",
    "ValueWidgetBuilder",
    "WidgetBuilder",
}

# A small, explicit set of compatibility/diagnostic APIs where preserving a
# named public equivalent would add little value before 1.0.  Other P3 rows
# remain classified as `DIRECT`/`RUSTIFIED` so the inventory still records
# their future compatibility surface.
LOW_VALUE_NAMES = {
    "CheckedModeBanner",
    "FlutterLogo",
    "FlutterLogoDecoration",
    "GridPaper",
    "PerformanceOverlay",
    "DiagnosticsNode",
    "DiagnosticLevel",
    "DebugCreator",
    "WidgetInspector",
    "WidgetInspectorService",
    "WidgetInspectorServiceExtensions",
    "DevToolsDeepLinkProperty",
    "InspectorButton",
    "InspectorButtonVariant",
    "InspectorReferenceData",
    "InspectorSelection",
    "InspectorSelectionChangedCallback",
    "InspectorSerializationDelegate",
    "EnableWidgetInspectorScope",
    "DisableWidgetInspectorScope",
    "MoveExitWidgetSelectionButtonBuilder",
    "ExitWidgetSelectionButtonBuilder",
    "TapBehaviorButtonBuilder",
}

# P0 is the deployable desktop core.  Keep this list capability-oriented and
# include domain values required to construct those widgets.  It is okay for
# a P0 row to remain deferred in the status column: that is precisely what
# the generated P0 work queue is meant to expose to implementation agents.
P0_NAMES = {
    # Layout and painting widgets.
    "Container", "Row", "Column", "Flex", "Expanded", "Flexible", "Spacer",
    "Stack", "Positioned", "IndexedStack", "Padding", "Align", "Center",
    "SizedBox", "ConstrainedBox", "LimitedBox", "UnconstrainedBox",
    "FractionallySizedBox", "AspectRatio", "FittedBox", "Wrap", "LayoutBuilder",
    "DecoratedBox", "ColoredBox", "Transform", "Opacity", "ClipRect", "ClipRRect",
    "ClipOval", "ClipPath", "BoxFit", "BoxDecoration", "Border", "BorderSide",
    "BorderRadius", "BorderStyle", "BoxShadow", "BoxShape", "Radius",
    "LinearGradient", "RadialGradient", "SweepGradient", "CustomPaint", "CustomPainter",
    "Canvas", "Path", "Color", "ColorFilter", "Paint", "Shader", "Shadow",
    # Text and text-domain values.
    "Text", "RichText", "TextStyle", "TextSpan", "InlineSpan", "WidgetSpan",
    "DefaultTextStyle", "TextOverflow", "TextAlign", "TextDirection", "StrutStyle",
    "TextBaseline", "TextDecoration", "TextDecorationStyle", "TextLeadingDistribution",
    "TextWidthBasis", "TextHeightBehavior", "TextScaler", "FontFeature", "FontVariation",
    "FontWeight", "TextShadow", "TextEditingController", "TextEditingValue", "TextSelection",
    "TextRange", "EditableText", "KeyboardListener", "SelectionContainer",
    # Images/icons (network image remains outside P0).
    "Image", "RawImage", "ImageIcon", "Icon", "IconData", "AssetImage", "MemoryImage",
    "FileImage", "ImageRepeat", "FilterQuality", "ImageProvider", "ImageConfiguration",
    # Pointer, gesture, focus, keyboard and typed command capability.
    "GestureDetector", "Listener", "MouseRegion", "PointerEvent", "TapDownDetails",
    "TapUpDetails", "DragDownDetails", "DragStartDetails", "DragUpdateDetails", "DragEndDetails",
    "ScaleStartDetails", "ScaleUpdateDetails", "ScaleEndDetails", "LongPressStartDetails",
    "LongPressMoveUpdateDetails", "LongPressEndDetails", "Focus", "FocusScope", "FocusNode",
    "FocusManager", "FocusTraversalGroup", "WidgetOrderTraversalPolicy",
    "ReadingOrderTraversalPolicy", "OrderedTraversalPolicy", "Shortcuts", "Actions", "Action",
    "Intent", "CallbackShortcuts", "ActionListener", "FocusableActionDetector",
    # Editing/forms.
    "Form", "FormField", "FormState", "FocusScopeNode", "UndoHistoryController",
    # Scrolling and the essential sliver surface.
    "Scrollable", "SingleChildScrollView", "ListView", "GridView", "PageView", "CustomScrollView",
    "ScrollController", "PageController", "ScrollPhysics", "PrimaryScrollController", "RawScrollbar",
    "AlwaysScrollableScrollPhysics", "BouncingScrollPhysics", "ClampingScrollPhysics",
    "NeverScrollableScrollPhysics", "PageScrollPhysics", "RangeMaintainingScrollPhysics",
    "SliverList", "SliverGrid", "SliverToBoxAdapter", "SliverPadding", "SliverFixedExtentList",
    "SliverVariedExtentList", "SliverFillRemaining", "SliverFillViewport", "SliverPersistentHeader",
    "PinnedHeaderSliver", "Viewport", "KeepAlive", "AutomaticKeepAlive",
    # Navigation/overlay and route composition.
    "Navigator", "Overlay", "OverlayEntry", "PopScope", "Router", "Route", "RouteSettings",
    "Page", "PageRoute", "PageRouteBuilder", "ModalRoute", "RawDialogRoute", "OverlayRoute",
    "TransitionRoute", "PopupRoute", "NavigatorObserver", "RestorationScope",
    # Environment, lifecycle, accessibility.
    "Directionality", "MediaQuery", "MediaQueryData", "SafeArea", "Localizations",
    "DefaultSelectionStyle", "Semantics", "MergeSemantics", "ExcludeSemantics", "BlockSemantics",
    "IndexedSemantics", "SemanticsDebugger", "TickerMode", "WidgetsApp",
    # Animation capability.
    "AnimationController", "Animation", "Animatable", "Tween", "Curve", "Curves",
    "FadeTransition", "SlideTransition", "ScaleTransition", "RotationTransition", "AnimatedContainer",
    "AnimatedOpacity", "AnimatedPadding", "AnimatedAlign", "AnimatedPositioned", "AnimatedSize",
    "AnimatedSwitcher", "AnimatedBuilder", "TweenAnimationBuilder",
}

P1_NAMES = {
    "IntrinsicWidth", "IntrinsicHeight", "Table", "TableCell", "Flow", "FlowDelegate",
    "InteractiveViewer", "Hero", "HeroMode", "HeroControllerScope", "AnimatedList", "AnimatedGrid",
    "ReorderableList", "Dismissible", "Draggable", "DragTarget", "NestedScrollView",
    "ListWheelScrollView", "SelectableRegion", "CompositedTransformTarget", "CompositedTransformFollower",
    "Tooltip", "RawTooltip", "SnapshotWidget", "NetworkImage", "FadeInImage", "ImageCache",
    "ImageStream", "ImageStreamCompleter", "Autocomplete", "RawAutocomplete", "ReorderableListState",
    "SliverAnimatedList", "SliverAnimatedGrid", "SliverReorderableList", "SliverPrototypeExtentList",
    "SliverSafeArea", "SliverVisibility", "SliverOpacity", "SliverIgnorePointer",
    "SelectionListener", "UndoHistory", "UndoHistoryController", "SelectableRegion",
    "OverlayPortal", "OverlayPortalController",
}

P2_NAMES = {
    "TwoDimensionalScrollView", "TwoDimensionalViewport", "TwoDimensionalScrollable",
    "TreeSliver", "TreeSliverController", "TreeSliverNode", "SliverCrossAxisGroup", "SliverMainAxisGroup",
    "SliverOverlapAbsorber", "SliverOverlapInjector", "SliverFloatingHeader", "StretchEffect",
    "SliverTree", "AdvancedMenuAnchor", "NestedScrollViewViewport", "ListWheelViewport",
    "InteractiveViewerWidgetBuilder", "TransformationController", "MagnifierController",
    "TextMagnifierConfiguration", "SnapshotController", "SnapshotMode", "SnapshotPainter",
    "RouteObserver", "RouteInformation", "RouteInformationParser", "RouterConfig",
    "RestorableRouteFuture", "RestorableRouteBuilder", "SelectableRegionContextMenuBuilder",
}


def port_policy_for(name: str, status: str, symbol_kind: str) -> str:
    """Return the Rust-facing policy independent of current implementation status."""

    if name in TASK24_POLICIES:
        return TASK24_POLICIES[name]
    if name in SIGNAL_MERGED:
        return "MERGED_SIGNAL"
    if name in CONTEXT_MERGED:
        return "MERGED_CONTEXT"
    if name in TOKIO_MERGED:
        return "MERGED_TOKIO"
    if name in INTERNAL_RUNTIME:
        return "INTERNAL"
    # The restoration hierarchy is merged into Incular's serde-backed
    # restoration data, rather than being treated as a Dart inheritance
    # mechanic. This check must precede the broad never-port architecture set.
    if name == "RestorationMixin" or name.startswith("Restorable") or name == "RestorationBucket":
        return "MERGED_DOMAIN"
    if name in NEVER_PORT_DART_ARCHITECTURE:
        return "SKIP_DART_MECHANIC"
    # Flutter's RenderObject/Element families are implementation architecture
    # even when a newly exported helper was not in the explicit baseline set.
    # Keep the guard lexical and source-independent so future graph additions
    # cannot accidentally turn one into a public compatibility target.
    if name.startswith("Render") or name.endswith("Element") or name.endswith("ElementMixin"):
        return "SKIP_DART_MECHANIC"
    if name in LOW_VALUE_NAMES or status == "SKIPPED_DEPRECATED":
        return "SKIP_LOW_VALUE"
    if status == "DEFERRED_PLATFORM":
        return "DEFER_PLATFORM"
    if name in DOMAIN_OWNERS or status == "MERGED":
        return "MERGED_DOMAIN"
    if name in CLOSURE_ALIASES:
        return "RUSTIFIED"
    if name in RUSTIFIED_CAPABILITIES:
        return "RUSTIFIED"
    if status == "SKIPPED_DART_MECHANIC":
        return "SKIP_DART_MECHANIC"
    if status == "DEFERRED_RENDERER":
        return "DEFER_RENDERER"
    if status == "RUSTIFIED":
        # Small, familiar widgets retain their Flutter-facing shape. More
        # stateful or callback-heavy APIs are recorded as Rustified below.
        direct = {
            "Widget", "Container", "Row", "Column", "Flex", "Expanded", "Flexible", "Spacer",
            "Stack", "Positioned", "IndexedStack", "Padding", "Align", "Center", "SizedBox",
            "ConstrainedBox", "LimitedBox", "UnconstrainedBox", "FractionallySizedBox", "AspectRatio",
            "FittedBox", "Wrap", "DecoratedBox", "ColoredBox", "Transform", "Opacity", "ClipRect",
            "ClipRRect", "ClipOval", "ClipPath", "Text", "Image", "RawImage", "ImageIcon", "Icon",
            "GestureDetector", "Listener", "MouseRegion", "Focus", "FocusScope", "FocusNode",
            "FocusManager", "Form", "FormField", "Scrollable", "SingleChildScrollView", "ListView",
            "GridView", "PageView", "CustomScrollView", "PageController", "PrimaryScrollController",
            "RawScrollbar", "Semantics", "MergeSemantics", "ExcludeSemantics", "BlockSemantics",
            "SafeArea", "Navigator", "Overlay", "OverlayEntry", "PopScope", "WidgetOrderTraversalPolicy",
            "ReadingOrderTraversalPolicy", "OrderedTraversalPolicy", "AnimationController", "Tween",
            "Curves", "FadeTransition", "SlideTransition", "ScaleTransition", "AnimatedContainer",
            "AnimatedOpacity", "AnimatedPadding", "AnimatedAlign", "AnimatedPositioned", "AnimatedSize",
            "AnimatedSwitcher", "BoxDecoration", "Border", "BorderSide", "CustomPaint", "CustomPainter",
        }
        return "DIRECT" if name in direct else "RUSTIFIED"
    if symbol_kind == "CALLBACK":
        return "RUSTIFIED"
    return "SKIP_LOW_VALUE"


def priority_for(name: str, port_policy: str) -> str:
    """Assign the implementation queue independent of the parity status."""

    if (
        name in NEVER_PORT_DART_ARCHITECTURE
        or name in CONTEXT_NEVER_PORT
        or name in SIGNAL_MERGED
        or name in INTERNAL_RUNTIME
        or port_policy == "SKIP_DART_MECHANIC"
    ):
        return "NEVER_PORT"
    if name.startswith("Restorable") or name in {"RestorationBucket", "RestorationMixin"}:
        return "NEVER_PORT"
    if name in {"FutureBuilder", "StreamBuilder", "StreamBuilderBase", "AsyncSnapshot", "ConnectionState"}:
        return "NEVER_PORT"
    if port_policy == "SKIP_LOW_VALUE" and name in LOW_VALUE_NAMES:
        return "P3_COMPATIBILITY"
    if name in P0_NAMES:
        return "P0_DEPLOYABLE"
    if name in P1_NAMES:
        return "P1_COMMON"
    if name in P2_NAMES:
        return "P2_ADVANCED"
    return "P3_COMPATIBILITY"


def policy_evidence(name: str, policy: str) -> str:
    replacements = {
        "MERGED_SIGNAL": "Signal dependency tracking is the sole general reactive primitive; no notifier/builder clone.",
        "MERGED_CONTEXT": "Typed context/environment dependencies replace Flutter's InheritedWidget hierarchy.",
        "MERGED_TOKIO": "Tokio task scopes plus Signal<Result/AsyncState> replace FutureBuilder/StreamBuilder mechanics.",
        "MERGED_DOMAIN": "The authoritative Incular/ecosystem domain crate owns this value or capability.",
        "INTERNAL": "Retained runtime/viewport machinery remains internal; applications use the declarative capability.",
        "SKIP_DART_MECHANIC": "Dart object-model/inheritance mechanic is intentionally not a public Incular API.",
        "SKIP_LOW_VALUE": "Low-value, diagnostic, deprecated, or compatibility-only surface is recorded without a port target.",
        "DEFER_PLATFORM": "A native platform adapter is required before this capability can be deployed.",
        "DEFER_RENDERER": "A retained renderer/compositor or domain prerequisite is required before this capability can be deployed.",
        "DIRECT": "The Flutter concept maps closely to the Incular Widgets API and remains a familiar public name.",
        "RUSTIFIED": "The capability is retained with Rust traits/closures/composition rather than Dart inheritance or aliases.",
    }
    return replacements[policy]


def priority_evidence(name: str, priority: str) -> str:
    if priority == "P0_DEPLOYABLE":
        return "Required for an ordinary deployable desktop application; include in the P0 closure queue."
    if priority == "P1_COMMON":
        return "Common application capability scheduled after the deployable P0 core is stable."
    if priority == "P2_ADVANCED":
        return "Advanced capability scheduled after P0/P1 or when a real application needs it."
    if priority == "NEVER_PORT":
        return "Dart-specific architecture or a deliberate Signal/Tokio/context replacement; never port mechanically."
    return "Compatibility/rare surface retained in the canonical inventory but not a pre-1.0 implementation priority."


def member_depth_for(
    status: str, application_facing: bool, name: str, symbol_kind: str
) -> str:
    """Record how the Dart member model was handled for a graph row.

    Implementation status and member depth are deliberately independent. A
    domain value that is re-exported through the Widgets namespace is merged
    into its authoritative crate, not claimed as a second Widgets member
    audit. Deferred rows likewise must not look complete merely because a
    compatibility name exists somewhere in the workspace.
    """

    if not application_facing or status in {"SKIPPED_DART_MECHANIC", "SKIPPED_DEPRECATED"}:
        return "SKIPPED"
    if status == "MERGED":
        return "MERGED_INTO"
    if status == "DEFERRED_PLATFORM":
        return "DEFERRED_PLATFORM"
    if status == "RUSTIFIED" and name in FULL_MEMBER_AUDIT_TYPES:
        return "FULL_MEMBER_AUDIT"
    if status == "RUSTIFIED" and symbol_kind in {"CONSTANT", "FUNCTION"}:
        return "NO_MEANINGFUL_APPLICATION_MEMBERS"
    # Renderer-deferred rows have an explicit status but no claimed member
    # implementation in this baseline.
    return "SKIPPED"


def evidence_for(
    status: str, member_depth: str, source: str, name: str
) -> tuple[str, str, str]:
    """Return member/default/behavior evidence without implying completeness."""

    if status == "MERGED":
        member = f"{source}: authoritative domain mapping; no duplicate Widgets member model"
        default = f"{source}: defaults audited by the owning domain crate"
        behavior = f"{source}: behavior audited by the owning domain crate"
    elif status == "RUSTIFIED" and member_depth == "FULL_MEMBER_AUDIT":
        member = f"specs/flutter_member_parity.jsonl: {name} rows are executable member evidence"
        default = f"{source}: defaults covered by the owning family tests"
        behavior = f"{source}: behavior covered by the owning family tests"
    elif status == "DEFERRED_PLATFORM":
        member = f"{source}: native platform members are explicitly deferred"
        default = f"{source}: platform default requires a native adapter"
        behavior = f"{source}: platform behavior requires a native adapter"
    elif status == "DEFERRED_RENDERER":
        member = f"{source}: member mapping deferred with the renderer prerequisite"
        default = f"{source}: default audit retained as a deferred prerequisite"
        behavior = f"{source}: behavior deferred with the renderer prerequisite"
    elif status in {"SKIPPED_DART_MECHANIC", "SKIPPED_DEPRECATED"}:
        member = f"{source}: intentionally skipped by status"
        default = f"{source}: intentionally skipped by status"
        behavior = f"{source}: intentionally skipped by status"
    else:
        member = f"{source}: facade/type mapping only; complete member audit is not claimed"
        default = f"{source}: default audit remains an explicit follow-up"
        behavior = f"{source}: behavior audit remains an explicit follow-up"
    return member, default, behavior


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--flutter-root",
        help="Flutter 3.47.1 checkout (or packages/flutter/lib directory)",
    )
    args = parser.parse_args()
    if args.flutter_root:
        set_flutter_root(args.flutter_root)
    elif not (FLUTTER / "widgets.dart").is_file():
        set_flutter_root(FLUTTER)

    exports = direct_exports()
    existing = rust_symbols()
    domain_symbols = domain_root_symbols()
    rows: list[dict[str, object]] = []
    graph_values = recursive_declarations("widgets.dart")
    # Keep the first declaration in export order. Duplicate exports are legal
    # in Dart and do not create duplicate public symbols.
    unique_values: list[tuple[str, str, str]] = []
    seen_names: set[str] = set()
    for name, kind, path in graph_values:
        if name in seen_names:
            continue
        seen_names.add(name)
        unique_values.append((name, kind, path))

    for name, kind, path in unique_values:
        deprecated = is_deprecated(path, name)
        status, owner, mapped = classify(
            name, path, kind, existing, domain_symbols, deprecated
        )
        application_facing = status not in {"SKIPPED_DART_MECHANIC", "SKIPPED_DEPRECATED"}
        symbol_kind = public_symbol_kind(name, kind, path, status)
        port_policy = port_policy_for(name, status, symbol_kind)
        priority = priority_for(name, port_policy)
        member_depth = member_depth_for(status, application_facing, name, symbol_kind)
        member_evidence, default_evidence, behavior_evidence = evidence_for(
            status, member_depth, path, name
        )
        rows.append(
            {
                "flutter_symbol": name,
                "flutter_source": f"packages/flutter/lib/{path}",
                "symbol_kind": symbol_kind,
                "deprecated": deprecated,
                "application_facing": application_facing,
                "platform_specific": status == "DEFERRED_PLATFORM",
                "incular_public_path": mapped,
                "incular_owner_crate": owner,
                "status": status,
                "p0_closure_status": TASK24_CLOSURE.get(name, (None, None, None, "NOT_IN_SCOPE"))[3],
                "port_policy": port_policy,
                "port_policy_evidence": policy_evidence(name, port_policy),
                "priority": priority,
                "priority_evidence": priority_evidence(name, priority),
                "member_depth": member_depth,
                "constructor_evidence": f"{path}: source declaration",
                "member_evidence": member_evidence,
                "default_evidence": default_evidence,
                "behavior_evidence": behavior_evidence,
                "test_evidence": "tests/widgets_3471_boundary.rs; family tests where implemented",
            }
        )

    # The export graph intentionally includes a few package dependencies that
    # are not part of the Flutter checkout's lib/ tree.
    rows.append(
        {
            "flutter_symbol": "Matrix4",
            "flutter_source": "package:vector_math/vector_math_64.dart",
            "symbol_kind": "VALUE_TYPE",
            "deprecated": False,
            "application_facing": True,
            "platform_specific": False,
            "incular_public_path": None,
            "incular_owner_crate": "incular-rendering",
            "status": "DEFERRED_RENDERER",
            "port_policy": "MERGED_DOMAIN",
            "port_policy_evidence": policy_evidence("Matrix4", "MERGED_DOMAIN"),
            "priority": "P2_ADVANCED",
            "priority_evidence": priority_evidence("Matrix4", "P2_ADVANCED"),
            "member_depth": "SKIPPED",
            "constructor_evidence": "external vector_math type; no affine 4x4 owner in the baseline",
            "member_evidence": "package:vector_math export is deferred until a renderer-neutral 4x4 mapping exists",
            "default_evidence": "package:vector_math default is deferred with the renderer mapping",
            "behavior_evidence": "package:vector_math behavior is deferred with the renderer mapping",
            "test_evidence": "P0_DEPLOYABLE_WIDGETS_SUMMARY.json deferred renderer inventory",
        }
    )
    for name, kind in (("Characters", "VALUE_TYPE"), ("CharacterRange", "VALUE_TYPE")):
        rows.append(
            {
                "flutter_symbol": name,
                "flutter_source": "package:characters/characters.dart",
                "symbol_kind": kind,
                "deprecated": False,
                "application_facing": True,
                "platform_specific": False,
                "incular_public_path": None,
                "incular_owner_crate": "characters",
                "status": "MERGED",
                "port_policy": "MERGED_DOMAIN",
                "port_policy_evidence": policy_evidence(name, "MERGED_DOMAIN"),
                "priority": "P3_COMPATIBILITY",
                "priority_evidence": priority_evidence(name, "P3_COMPATIBILITY"),
                "member_depth": "MERGED_INTO",
                "constructor_evidence": "external Dart package export",
                "member_evidence": "external Dart package export",
                "default_evidence": "N/A: external value type",
                "behavior_evidence": "Parley/Unicode domain ownership",
                "test_evidence": "tests/widgets_3471_surface.rs",
            }
        )

    rows.sort(key=lambda row: (str(row["flutter_symbol"]), str(row["flutter_source"])))
    counts: dict[str, int] = {}
    kind_counts: dict[str, int] = {}
    for row in rows:
        counts[str(row["status"])] = counts.get(str(row["status"]), 0) + 1
        kind = str(row["symbol_kind"])
        kind_counts[kind] = kind_counts.get(kind, 0) + 1
    application_facing_count = sum(bool(row["application_facing"]) for row in rows)
    platform_specific_count = sum(bool(row["platform_specific"]) for row in rows)
    deprecated_count = sum(bool(row["deprecated"]) for row in rows)
    policy_counts: dict[str, int] = {}
    priority_counts: dict[str, int] = {}
    for row in rows:
        policy = str(row["port_policy"])
        priority = str(row["priority"])
        policy_counts[policy] = policy_counts.get(policy, 0) + 1
        priority_counts[priority] = priority_counts.get(priority, 0) + 1
    if set(policy_counts) != PORT_POLICIES:
        missing = sorted(PORT_POLICIES - set(policy_counts))
        raise SystemExit(f"port-policy taxonomy has no rows for: {', '.join(missing)}")
    if set(priority_counts) != PRIORITIES:
        missing = sorted(PRIORITIES - set(priority_counts))
        raise SystemExit(f"priority taxonomy has no rows for: {', '.join(missing)}")
    p0_rows = [row for row in rows if row["priority"] == "P0_DEPLOYABLE"]
    p0_application_rows = [row for row in p0_rows if row["application_facing"]]
    p0_actionable_rows = [
        row
        for row in p0_rows
        if row["port_policy"]
        not in {"INTERNAL", "SKIP_DART_MECHANIC", "SKIP_LOW_VALUE", "DEFER_PLATFORM"}
    ]
    p0_status_counts: dict[str, int] = {}
    p0_policy_counts: dict[str, int] = {}
    p0_closure_counts: dict[str, int] = {}
    for row in p0_rows:
        status = str(row["status"])
        policy = str(row["port_policy"])
        p0_status_counts[status] = p0_status_counts.get(status, 0) + 1
        p0_policy_counts[policy] = p0_policy_counts.get(policy, 0) + 1
        closure = str(row.get("p0_closure_status", "NOT_IN_SCOPE"))
        p0_closure_counts[closure] = p0_closure_counts.get(closure, 0) + 1
    task24_rows = [row for row in p0_rows if row.get("p0_closure_status") != "NOT_IN_SCOPE"]
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "flutter_widgets_3471_exports.json").write_text(
        json.dumps(
            {
                "flutter_tag": "3.47.1",
                "flutter_commit": COMMIT,
                "dart_version": "3.13.1",
                "previous_tag": "3.47.0",
                "previous_commit": PREVIOUS_COMMIT,
                "source": "packages/flutter/lib/widgets.dart",
                "direct_export_count": len(exports),
                "recursive_symbol_count": len(rows),
                "status_counts": counts,
                "symbol_kind_counts": kind_counts,
                "application_facing_count": application_facing_count,
                "platform_specific_count": platform_specific_count,
                "deprecated_count": deprecated_count,
                "port_policy_counts": policy_counts,
                "priority_counts": priority_counts,
                "p0_deployable_count": len(p0_rows),
                "p0_application_facing_count": len(p0_application_rows),
                "p0_actionable_count": len(p0_actionable_rows),
                "exports": exports,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    with (OUT / "flutter_widgets_3471_parity.jsonl").open("w", encoding="utf-8") as stream:
        for row in rows:
            stream.write(json.dumps(row, sort_keys=True) + "\n")
    with (OUT / "P0_DEPLOYABLE_WIDGETS.jsonl").open("w", encoding="utf-8") as stream:
        for row in p0_rows:
            stream.write(json.dumps(row, sort_keys=True) + "\n")
    p0_summary = {
        "artifact": "specs/P0_DEPLOYABLE_WIDGETS.jsonl",
        "canonical_manifest": "specs/flutter_widgets_3471_parity.jsonl",
        "flutter_tag": "3.47.1",
        "flutter_commit": COMMIT,
        "dart_version": "3.13.1",
        "canonical_row_count": len(rows),
        "application_facing_count": application_facing_count,
        "p0_deployable_count": len(p0_rows),
        "p0_application_facing_count": len(p0_application_rows),
        "p0_actionable_count": len(p0_actionable_rows),
        "p0_status_counts": p0_status_counts,
        "p0_port_policy_counts": p0_policy_counts,
        "p0_closure_counts": p0_closure_counts,
        "p0_closure_expected_count": len(TASK24_CLOSURE),
        "p0_closure_complete_count": sum(
            count for closure, count in p0_closure_counts.items() if closure != "TRUE_RENDERER_DEFERRED"
        ) - p0_closure_counts.get("NOT_IN_SCOPE", 0),
        "p0_true_deferred_count": p0_closure_counts.get("TRUE_RENDERER_DEFERRED", 0),
        "p0_remaining_deferred_symbols": [
            row["flutter_symbol"] for row in task24_rows if row.get("p0_closure_status") == "TRUE_RENDERER_DEFERRED"
        ],
        "status_counts": counts,
        "port_policy_counts": policy_counts,
        "priority_counts": priority_counts,
        "priority_values": sorted(PRIORITIES),
        "port_policy_values": sorted(PORT_POLICIES),
    }
    (OUT / "P0_DEPLOYABLE_WIDGETS_SUMMARY.json").write_text(
        json.dumps(p0_summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    report = [
        "# Flutter Widgets 3.47.1 parity gaps",
        "",
        f"- Flutter tag: `3.47.1` ({COMMIT})",
        "- Dart: `3.13.1`",
        f"- Direct export directives: {len(exports)}",
        f"- Canonical rows: {len(rows)}",
        f"- Application-facing rows: {application_facing_count}",
        f"- Supporting/framework rows: {len(rows) - application_facing_count}",
        f"- Platform-specific rows: {platform_specific_count}",
        f"- Deprecated rows: {deprecated_count}",
        f"- Symbol kinds: {json.dumps(kind_counts, sort_keys=True)}",
        f"- Status counts: {json.dumps(counts, sort_keys=True)}",
        f"- Port-policy counts: {json.dumps(policy_counts, sort_keys=True)}",
        f"- Priority counts: {json.dumps(priority_counts, sort_keys=True)}",
        f"- P0 deployable rows: {len(p0_rows)} ({len(p0_application_rows)} application-facing; {len(p0_actionable_rows)} actionable)",
        "- P0 queue: `specs/P0_DEPLOYABLE_WIDGETS.jsonl` with summary metadata in `specs/P0_DEPLOYABLE_WIDGETS_SUMMARY.json`",
        "",
        "This report is generated from the exact `widgets.dart` export graph. API-page hyperlinks are not inventory inputs.",
        "",
        "## Boundary contract",
        "",
        "`incular-widgets` owns only Flutter Widgets concepts. Domain values are implemented once in their authoritative crates and re-exported by the facade. Headless controls remain in `incular-controls`; Material remains in `incular-material`.",
        "",
        "## Deferred prerequisites",
        "",
        "Rows marked `DEFERRED_RENDERER` require a retained renderer/compositor or an unfinished domain prerequisite; rows marked `DEFERRED_PLATFORM` require a native platform adapter. Dart framework inheritance mechanics are explicitly `SKIPPED_DART_MECHANIC`, not claimed as application widgets.",
        "",
        "## Task 23 policy and priority contract",
        "",
        "`port_policy` records the Rust-native decision independently of implementation status: capability names may be `RUSTIFIED` or `DIRECT` while their current status remains deferred. `priority` records the implementation queue; `NEVER_PORT` is reserved for Dart architecture replaced by Signals, typed context, Tokio, retained controllers, or Incular's retained arena.",
        "",
        "## Status detail",
        "",
    ]
    for status in sorted(counts):
        names = [str(row["flutter_symbol"]) for row in rows if row["status"] == status]
        report.append(f"### {status} ({len(names)})")
        report.append("")
        report.append(", ".join(f"`{name}`" for name in names))
        report.append("")
    report.extend(["## Priority detail", ""])
    for priority in ["P0_DEPLOYABLE", "P1_COMMON", "P2_ADVANCED", "P3_COMPATIBILITY", "NEVER_PORT"]:
        names = [str(row["flutter_symbol"]) for row in rows if row["priority"] == priority]
        report.append(f"### {priority} ({len(names)})")
        report.append("")
        report.append(", ".join(f"`{name}`" for name in names))
        report.append("")
    report.extend(["## Port-policy detail", ""])
    for policy in sorted(policy_counts):
        names = [str(row["flutter_symbol"]) for row in rows if row["port_policy"] == policy]
        report.append(f"### {policy} ({len(names)})")
        report.append("")
        report.append(", ".join(f"`{name}`" for name in names))
        report.append("")
    # Keep generated prose in ignored output; checked-in JSONL is the durable
    # parity source of truth and the repository root stays documentation-light.
    REPORT_DIR.mkdir(parents=True, exist_ok=True)
    (REPORT_DIR / "WIDGETS_3471_GAPS.md").write_text("\n".join(report), encoding="utf-8")
    print(json.dumps({"rows": len(rows), "statuses": counts}, sort_keys=True))


if __name__ == "__main__":
    main()
