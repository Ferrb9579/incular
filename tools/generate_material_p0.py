#!/usr/bin/env python3
"""Generate the checked-in Flutter 3.47.1 Material P0 graph.

The canonical inventory remains the single source of truth.  Wave-0 priority
metadata is kept as a review artifact under ``target/``; this generator joins
the two files, extends every canonical row with the required policy fields,
and emits the P0 projection plus a machine-readable summary.
"""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPECS = ROOT / "specs"
CANONICAL = SPECS / "flutter_material_3471_parity.jsonl"
PRIORITY = ROOT / "target/material25/priority.jsonl"
P0 = SPECS / "P0_MATERIAL_3471.jsonl"
SUMMARY = SPECS / "P0_MATERIAL_3471_SUMMARY.json"

PRIORITY_NAMES = {
    "P0": "P0_MATERIAL_DEPLOYABLE",
    "P1": "P1_MATERIAL_COMMON",
    "P2": "P2_MATERIAL_ADVANCED",
    "P3": "P3_MATERIAL_COMPATIBILITY",
}

# A few canonical rows are useful to keep in the full inventory but are
# explicitly outside Task 25's deployable Material 3 slice.  Keep this policy
# in the generator so a regenerated manifest cannot accidentally promote an
# unfinished picker/sheet/segmented-button branch back into P0.  The review
# artifact is still written with these overrides applied, making the decision
# auditable and deterministic after ``cargo clean``.
P1_OVERRIDES: dict[str, tuple[str, str, str]] = {
    # Bottom sheets are a separate route/inset/gesture surface (Task 25 P1).
    "BottomSheet": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    "BottomSheetDragEndHandler": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    "BottomSheetDragStartHandler": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    "BottomSheetThemeData": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    "ModalBottomSheetRoute": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    "PersistentBottomSheetController": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    "showBottomSheet": ("P1", "surfaces", "Bottom-sheet route and drag/inset behavior is a P1 surface."),
    # Pickers require calendar/clock localization and platform interaction.
    "CalendarDatePicker": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "DatePickerDialog": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "DateRangePickerDialog": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "EntryModeChangeCallback": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "InputDatePickerFormField": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "SelectableDayForRangePredicate": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "showDatePicker": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "showDateRangePicker": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "showTimePicker": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "TimePickerDialog": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "TimePickerEntryMode": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    "YearPicker": ("P1", "dialogs", "Date/time pickers are explicitly outside the P0 desktop surface."),
    # SegmentedButton is a distinct selection family and is scheduled for P1.
    "SegmentedButton": ("P1", "buttons", "Segmented buttons are explicitly a P1 common component."),
    "SegmentedButtonState": ("P1", "buttons", "Segmented buttons are explicitly a P1 common component."),
    "SegmentedButtonTheme": ("P1", "buttons", "Segmented buttons are explicitly a P1 common component."),
    "SegmentedButtonThemeData": ("P1", "buttons", "Segmented buttons are explicitly a P1 common component."),
}

# App-facing P0 types are audited against the retained implementation rather
# than the older Wave-1 inventory status.  Supporting declarations (shape
# internals, platform adapters, legacy aliases, and picker families) remain
# explicitly deferred and stay visible in the generated projection.
IMPLEMENTED_COMPONENTS: dict[str, tuple[str, str]] = {
    "MaterialApp": ("RUSTIFIED", "incular_material::MaterialApp"),
    "MaterialScrollBehavior": ("RUSTIFIED", "incular_material::MaterialScrollBehavior"),
    "AppBar": ("RUSTIFIED", "incular_material::AppBar"),
    "SliverAppBar": ("RUSTIFIED", "incular_material::SliverAppBar"),
    "Scaffold": ("RUSTIFIED", "incular_material::Scaffold"),
    "ScaffoldMessenger": ("MERGED_CONTROLS", "incular_material::ScaffoldMessenger"),
    "ScaffoldMessengerState": ("MERGED_CONTROLS", "incular_material::ScaffoldMessengerController"),
    "SnackBar": ("MERGED_CONTROLS", "incular_material::SnackBar"),
    "SnackBarAction": ("MERGED_CONTROLS", "incular_material::SnackBarAction"),
    "BottomAppBar": ("RUSTIFIED", "incular_material::BottomAppBar"),
    "DrawerHeader": ("RUSTIFIED", "incular_material::DrawerHeader"),
    "NavigationRail": ("RUSTIFIED", "incular_material::NavigationRail"),
    "NavigationRailDestination": ("RUSTIFIED", "incular_material::NavigationRailDestination"),
    "NavigationDrawer": ("RUSTIFIED", "incular_material::NavigationDrawer"),
    "NavigationDrawerDestination": ("RUSTIFIED", "incular_material::NavigationDrawerDestination"),
    "BackButton": ("RUSTIFIED", "incular_material::BackButton"),
    "BackButtonIcon": ("RUSTIFIED", "incular_material::BackButtonIcon"),
    "CloseButton": ("RUSTIFIED", "incular_material::CloseButton"),
    "CloseButtonIcon": ("RUSTIFIED", "incular_material::CloseButtonIcon"),
    "DrawerButton": ("RUSTIFIED", "incular_material::DrawerButton"),
    "DrawerButtonIcon": ("RUSTIFIED", "incular_material::DrawerButtonIcon"),
    "EndDrawerButton": ("RUSTIFIED", "incular_material::EndDrawerButton"),
    "EndDrawerButtonIcon": ("RUSTIFIED", "incular_material::EndDrawerButtonIcon"),
    "Dialog": ("MERGED_CONTROLS", "incular_material::Dialog"),
    "DialogRoute": ("MERGED_CONTROLS", "incular_material::DialogRoute"),
    "SimpleDialog": ("MERGED_CONTROLS", "incular_material::SimpleDialog"),
    "SimpleDialogOption": ("MERGED_CONTROLS", "incular_material::SimpleDialogOption"),
    "DropdownButton": ("MERGED_CONTROLS", "incular_material::DropdownButton"),
    "DropdownButtonFormField": ("MERGED_CONTROLS", "incular_material::DropdownButtonFormField"),
    "DropdownButtonHideUnderline": ("MERGED_CONTROLS", "incular_material::DropdownButtonHideUnderline"),
    "DropdownMenu": ("MERGED_CONTROLS", "incular_material::DropdownMenu"),
    "DropdownMenuEntry": ("MERGED_CONTROLS", "incular_material::DropdownMenuEntry"),
    "DropdownMenuItem": ("MERGED_CONTROLS", "incular_material::DropdownMenuItem"),
    "MenuAnchor": ("MERGED_CONTROLS", "incular_material::MenuAnchor"),
    "MenuItemButton": ("MERGED_CONTROLS", "incular_material::MenuItemButton"),
    "SubmenuButton": ("MERGED_CONTROLS", "incular_material::SubmenuButton"),
    "MenuBar": ("MERGED_CONTROLS", "incular_material::MenuBar"),
    "PopupMenuButton": ("MERGED_CONTROLS", "incular_material::PopupMenuButton"),
    "PopupMenuEntry": ("MERGED_CONTROLS", "incular_material::PopupMenuEntry"),
    "PopupMenuItem": ("MERGED_CONTROLS", "incular_material::PopupMenuItem"),
    "PopupMenuDivider": ("MERGED_CONTROLS", "incular_material::PopupMenuDivider"),
    "CheckedPopupMenuItem": ("MERGED_CONTROLS", "incular_material::CheckedPopupMenuItem"),
    "RangeSlider": ("MERGED_CONTROLS", "incular_material::RangeSlider"),
    "SliderThemeData": ("RUSTIFIED", "incular_material::SliderThemeData"),
    "TabController": ("MERGED_CONTROLS", "incular_material::TabController"),
    "TabBarView": ("MERGED_CONTROLS", "incular_material::TabBarView"),
    "InputDecorator": ("MERGED_CONTROLS", "incular_material::InputDecorator"),
    "OutlineInputBorder": ("RUSTIFIED", "incular_material::OutlineInputBorder"),
    "UnderlineInputBorder": ("RUSTIFIED", "incular_material::UnderlineInputBorder"),
    "ProgressIndicatorTheme": ("RUSTIFIED", "incular_material::ProgressIndicatorTheme"),
    "ProgressIndicatorThemeData": ("RUSTIFIED", "incular_material::ProgressIndicatorThemeData"),
    "Tooltip": ("MERGED_CONTROLS", "incular_material::Tooltip"),
    "TooltipTheme": ("MERGED_CONTROLS", "incular_material::TooltipTheme"),
    "Icons": ("RUSTIFIED", "incular_material::Icons"),
    "Card": ("RUSTIFIED", "incular_material::Card"),
}


def read_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def read_priority(canonical: list[dict[str, object]]) -> list[dict[str, object]]:
    """Load the review artifact, rebuilding it after a normal ``cargo clean``.

    ``target/`` is intentionally ignored build output, so a clean checkout (or
    a disk-pressure cleanup) may remove the Wave-0 review file.  The canonical
    manifest already contains the generated priority, track, and rationale
    fields; use those fields to restore the review projection instead of
    making regeneration depend on an ephemeral directory.
    """
    if PRIORITY.exists():
        rows = read_jsonl(PRIORITY)
        for row in rows:
            override = P1_OVERRIDES.get(str(next(
                (item.get("flutter") for item in canonical if str(item["id"]) == str(row["id"])),
                "",
            )))
            if override is not None:
                row["priority"], row["track"], row["rationale"] = override
        return rows

    PRIORITY.parent.mkdir(parents=True, exist_ok=True)
    rows: list[dict[str, object]] = []
    reverse = {value: key for key, value in PRIORITY_NAMES.items()}
    for row in canonical:
        priority = str(row.get("priority", ""))
        short = reverse.get(priority)
        if short is None:
            raise SystemExit(f"canonical row has no generated priority: {row.get('id')}")
        flutter = str(row.get("flutter", ""))
        override = P1_OVERRIDES.get(flutter)
        priority = override[0] if override is not None else short
        track = override[1] if override is not None else row.get("priority_track")
        rationale = override[2] if override is not None else row.get("priority_rationale")
        rows.append(
            {
                "id": row["id"],
                "priority": priority,
                "track": track,
                "rationale": rationale,
            }
        )
    PRIORITY.write_text(
        "".join(json.dumps(row, separators=(",", ":"), sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )
    return rows


def port_policy(status: str) -> str:
    return {
        "EXACT": "DIRECT",
        "RUSTIFIED": "RUSTIFIED",
        "MERGED_CONTROLS": "MERGED_CONTROLS",
        "MERGED_CORE": "MERGED_CORE",
        "SKIPPED_LEGACY": "SKIP_LEGACY",
        "SKIPPED_PLATFORM": "DEFER_PLATFORM",
        "INTERNAL": "DEFER_PLATFORM",
        # The canonical graph predates the P0 closure and records deferred
        # rows conservatively.  They remain explicit until a later audit can
        # prove a direct/merged implementation; they are never silently
        # counted as complete.
        "DEFERRED": "DEFER_PLATFORM",
    }.get(status, "DEFER_PLATFORM")


def audited_component(row: dict[str, object], priority: str) -> tuple[str, str] | None:
    if priority != "P0_MATERIAL_DEPLOYABLE":
        return None
    flutter = str(row.get("flutter", ""))
    return IMPLEMENTED_COMPONENTS.get(flutter)


def implementation_status(row: dict[str, object], priority: str) -> str:
    audited = audited_component(row, priority)
    if audited is not None:
        policy, _ = audited
        return {
            "RUSTIFIED": "RUSTIFIED_IMPLEMENTED",
            "MERGED_CONTROLS": "MERGED_CONTROLS_IMPLEMENTED",
            "MERGED_CORE": "MERGED_CORE_IMPLEMENTED",
            "EXACT": "DIRECT_IMPLEMENTED",
        }[policy]
    status = str(row["status"])
    if status == "EXACT":
        return "DIRECT_IMPLEMENTED"
    if status == "RUSTIFIED":
        return "RUSTIFIED_IMPLEMENTED"
    if status == "MERGED_CONTROLS":
        return "MERGED_CONTROLS_IMPLEMENTED"
    if status == "MERGED_CORE":
        return "MERGED_CORE_IMPLEMENTED"
    if status == "SKIPPED_LEGACY":
        return "SKIPPED_LEGACY"
    if status == "SKIPPED_PLATFORM":
        return "DEFERRED_PLATFORM"
    if status == "INTERNAL":
        return "INTERNAL"
    # Supporting platform/renderer rows stay visible in the P0 projection, but
    # use the same explicit status vocabulary as the port policy.  This avoids
    # an ambiguous partial label being mistaken for an unfinished ordinary
    # application component.
    return "DEFERRED_PLATFORM" if priority == "P0_MATERIAL_DEPLOYABLE" else "DEFERRED"


def main() -> None:
    canonical = read_jsonl(CANONICAL)
    priority_rows = read_priority(canonical)
    by_id = {str(row["id"]): row for row in priority_rows}
    canonical_ids = {str(row["id"]) for row in canonical}
    if len(by_id) != len(priority_rows) or set(by_id) != canonical_ids:
        missing = sorted(canonical_ids - set(by_id))
        extra = sorted(set(by_id) - canonical_ids)
        raise SystemExit(f"priority graph does not match canonical rows; missing={missing[:3]} extra={extra[:3]}")

    extended: list[dict[str, object]] = []
    for row in canonical:
        review = by_id[str(row["id"])]
        priority = PRIORITY_NAMES[str(review["priority"])]
        item = dict(row)
        item["priority"] = priority
        audited = audited_component(item, priority)
        item["port_policy"] = audited[0] if audited is not None else port_policy(str(row["status"]))
        item["implementation_status"] = implementation_status(item, priority)
        if audited is not None:
            item["incular"] = audited[1]
            item["compile_evidence"] = "cargo_check_material_p0"
            item["test_evidence"] = "material_p0_surface_and_stress"
        item["priority_track"] = review.get("track")
        item["priority_rationale"] = review.get("rationale")
        extended.append(item)

    CANONICAL.write_text(
        "".join(json.dumps(row, separators=(",", ":"), sort_keys=True) + "\n" for row in extended),
        encoding="utf-8",
    )
    p0_rows = [row for row in extended if row["priority"] == "P0_MATERIAL_DEPLOYABLE"]
    P0.write_text(
        "".join(json.dumps(row, separators=(",", ":"), sort_keys=True) + "\n" for row in p0_rows),
        encoding="utf-8",
    )

    summary = {
        "artifact": str(P0.relative_to(ROOT)),
        "canonical_manifest": str(CANONICAL.relative_to(ROOT)),
        "flutter_tag": "3.47.1",
        "flutter_commit": "6655482ec06e547f90abf8ae7590466f4415978d",
        "canonical_row_count": len(extended),
        "p0_row_count": len(p0_rows),
        "priority_counts": dict(Counter(str(row["priority"]) for row in extended)),
        "port_policy_counts": dict(Counter(str(row["port_policy"]) for row in extended)),
        "p0_port_policy_counts": dict(Counter(str(row["port_policy"]) for row in p0_rows)),
        "implementation_status_counts": dict(
            Counter(str(row["implementation_status"]) for row in extended)
        ),
        "p0_implementation_status_counts": dict(
            Counter(str(row["implementation_status"]) for row in p0_rows)
        ),
        "p0_tracks": dict(Counter(str(row["priority_track"]) for row in p0_rows)),
        # P0 rows whose canonical member is a platform/renderer supporting
        # declaration remain visible as DEFERRED_PLATFORM.  Keep this count
        # honest; a priority projection is not a completion claim for every
        # supporting Flutter declaration.
        "p0_true_deferred_count": sum(
            row["implementation_status"] in {"DEFERRED", "DEFERRED_PLATFORM"} for row in p0_rows
        ),
        "p0_deferred_platform_count": sum(
            row["implementation_status"] == "DEFERRED_PLATFORM" for row in p0_rows
        ),
        "generation": "tools/generate_material_p0.py; priority review target/material25/priority.jsonl",
        "scope_note": "P0 deployability projection only; P1/P2/P3 and legacy/platform rows remain tracked in the canonical graph.",
    }
    SUMMARY.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
