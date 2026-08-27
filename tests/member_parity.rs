use std::{collections::HashSet, fs, path::Path, rc::Rc, time::Duration};

use incular::prelude::*;
use incular::scroll::{BoundaryPhysics, Scrollability};
use incular::widgets::internal::{
    FilteringTextInputFormatter, LengthLimitingTextInputFormatter,
    TextEditingValue as RetainedTextEditingValue, TextInputFormatter,
    TextSelection as RetainedTextSelection,
};
use serde_json::Value;

#[test]
fn test_member_parity_manifest_is_100_percent_resolved() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = repo_root.join("specs/flutter_stable_api_snapshot.json");
    let manifest_path = repo_root.join("specs/flutter_member_parity.jsonl");

    assert!(snapshot_path.exists(), "Snapshot file must exist");
    assert!(manifest_path.exists(), "Member parity manifest must exist");

    let snapshot_content = fs::read_to_string(&snapshot_path).expect("Read snapshot");
    let snapshot_json: Value =
        serde_json::from_str(&snapshot_content).expect("Parse snapshot JSON");

    let canonical_types: HashSet<String> = snapshot_json["types"]
        .as_array()
        .expect("types array")
        .iter()
        .map(|v| v.as_str().expect("type name string").to_string())
        .collect();

    let manifest_content = fs::read_to_string(&manifest_path).expect("Read manifest");
    let mut covered_types = HashSet::new();
    let mut member_count = 0;

    for line in manifest_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: Value = serde_json::from_str(trimmed).expect("Parse manifest line");
        let status = record["status"].as_str().expect("status string");
        assert_eq!(status, "implemented", "Member status must be implemented");

        let incular_type = record["incular_type"]
            .as_str()
            .expect("incular_type string");
        covered_types.insert(incular_type.to_string());
        member_count += 1;
    }

    assert!(
        member_count >= 400,
        "Must audit at least 400 canonical members"
    );

    let missing_types: Vec<_> = canonical_types.difference(&covered_types).collect();
    assert!(
        missing_types.is_empty(),
        "Missing coverage for canonical types: {:?}",
        missing_types
    );
}

#[test]
fn test_text_style_deep_parity_and_semantics() {
    let base = TextStyle::new()
        .font_size(14.0)
        .color(Color::rgba(10, 20, 30, 255))
        .font_weight(FontWeight::NORMAL);

    let overlay = TextStyle::new()
        .font_size(18.0)
        .font_weight(FontWeight::BOLD)
        .color(Color::rgba(50, 60, 70, 255));

    let merged = base.merge(&overlay);
    assert_eq!(merged.size, 18.0);
    assert_eq!(merged.weight, FontWeight::BOLD);
    assert_eq!(merged.color, Color::rgba(50, 60, 70, 255));

    // Scaling
    let scaled = base.apply(None, None, Some(1.5), None, None, None);
    assert_eq!(scaled.size, 21.0);

    // Interpolation (Lerp)
    let a = TextStyle::new()
        .font_size(10.0)
        .color(Color::rgba(0, 0, 0, 255));
    let b = TextStyle::new()
        .font_size(20.0)
        .color(Color::rgba(100, 100, 100, 255));
    let mid = a.lerp(&b, 0.5);
    assert_eq!(mid.size, 15.0);

    // Invalidation (ChangeImpact)
    let color_changed = base.clone().color(Color::rgba(255, 0, 0, 255));
    assert_eq!(base.change_impact(&color_changed), ChangeImpact::Paint);

    let size_changed = TextStyle::new().font_size(24.0);
    assert_eq!(base.change_impact(&size_changed), ChangeImpact::Layout);
}

#[test]
fn test_geometry_and_insets_parity() {
    let insets = EdgeInsets::symmetric(20.0, 10.0);
    assert_eq!(insets.left, 20.0);
    assert_eq!(insets.top, 10.0);
    assert_eq!(insets.right, 20.0);
    assert_eq!(insets.bottom, 10.0);

    let added = insets + EdgeInsets::all(5.0);
    assert_eq!(added.left, 25.0);
    assert_eq!(added.top, 15.0);

    let directional = EdgeInsetsDirectional::from_ste_b(10.0, 5.0, 20.0, 15.0);
    let resolved_ltr = directional.resolve(TextDirection::Ltr);
    assert_eq!(resolved_ltr.left, 10.0);
    assert_eq!(resolved_ltr.right, 20.0);

    let resolved_rtl = directional.resolve(TextDirection::Rtl);
    assert_eq!(resolved_rtl.left, 20.0);
    assert_eq!(resolved_rtl.right, 10.0);

    let align_dir = AlignmentDirectional::TOP_START;
    assert_eq!(align_dir.resolve(TextDirection::Ltr), Alignment::TOP_LEFT);
    assert_eq!(align_dir.resolve(TextDirection::Rtl), Alignment::TOP_RIGHT);

    let frac = FractionalOffset::new(0.5, 0.5);
    assert_eq!(frac.to_alignment(), Alignment::CENTER);
}

#[test]
fn test_box_decoration_and_border_parity() {
    let side = BorderSide::new(Color::rgba(0, 0, 0, 255), 2.0, BorderStyle::Solid);
    let border = BoxBorder::all(side);
    assert_eq!(border.dimensions(), EdgeInsets::all(2.0));

    let radius = BorderRadius::circular(8.0);
    let radii = radius.to_corner_radii();
    assert_eq!(radii.top_left, 8.0);
    assert_eq!(radii.bottom_right, 8.0);

    let dec = BoxDecoration::new()
        .color(Color::WHITE)
        .border(border)
        .border_radius(radius)
        .box_shadow(vec![BoxShadow::new(
            Color::BLACK,
            Offset::new(0.0, 2.0),
            4.0,
            0.0,
        )]);

    assert!(dec.is_valid());
}

#[test]
fn test_scroll_physics_and_metrics_parity() {
    let physics = ScrollPhysics::default()
        .bouncing()
        .then(ScrollPhysics::default().always_scrollable());
    assert!(matches!(physics.boundary, BoundaryPhysics::Bouncing { .. }));
    assert_eq!(physics.scrollability, Scrollability::Always);

    let metrics = ScrollMetrics {
        pixels: 100.0,
        min_scroll_extent: 0.0,
        max_scroll_extent: 500.0,
        viewport_dimension: 200.0,
        axis: Axis::Vertical,
        axis_direction: AxisDirection::Down,
        device_pixel_ratio: 1.0,
    };

    assert_eq!(metrics.extent_before(), 100.0);
    assert_eq!(metrics.extent_inside(), 200.0);
    assert_eq!(metrics.extent_after(), 400.0);
    assert_eq!(metrics.extent_total(), 700.0);
    assert!(!metrics.at_edge());
    assert!(!metrics.out_of_range());
}

#[test]
fn test_animation_and_physics_simulations() {
    let spring = SpringDescription::with_damping_ratio(1.0, 100.0, 1.0);
    assert_eq!(spring.spring_type(), SpringType::CriticallyDamped);

    let sim = SpringSimulation::new(spring, 0.0, 100.0, 0.0, Tolerance::DEFAULT);
    let pos0 = sim.position(Duration::ZERO);
    let pos_end = sim.position(Duration::from_secs(10));
    assert!((pos0 - 0.0).abs() < 1e-3);
    assert!((pos_end - 100.0).abs() < 1.0);

    let curve = Curves::EASE_IN_OUT;
    assert_eq!(curve.apply(0.0), 0.0);
    assert_eq!(curve.apply(1.0), 1.0);

    let reversed = curve.reverse();
    assert_eq!(reversed.apply(0.0), 0.0);
    assert_eq!(reversed.apply(1.0), 1.0);
}

#[test]
fn test_focus_and_traversal_policies() {
    let node1 = FocusNode::new();
    let node2 = FocusNode::new();
    let node3 = FocusNode::new();

    node2.set_skip_traversal(true);

    let policy = WidgetOrderTraversalPolicy;
    let nodes = [node1.clone(), node2.clone(), node3.clone()];

    let first = policy.find_first_focus(&nodes);
    assert_eq!(first, Some(node1.clone()));

    let next = policy.next(&node1, &nodes);
    assert_eq!(next, Some(node3.clone())); // node2 skipped

    let last = policy.find_last_focus(&nodes);
    assert_eq!(last, Some(node3.clone()));
}

#[test]
fn test_forms_and_text_formatters() {
    let digit_formatter = FilteringTextInputFormatter::digits_only();
    let old_val = RetainedTextEditingValue {
        text: "123".into(),
        selection: RetainedTextSelection::collapsed(3),
        composing: None,
    };
    let new_val = RetainedTextEditingValue {
        text: "123abc45".into(),
        selection: RetainedTextSelection::collapsed(8),
        composing: None,
    };
    let formatted = digit_formatter.format_edit_update(&old_val, &new_val);
    assert_eq!(formatted.text, "12345");

    let limit_formatter = LengthLimitingTextInputFormatter::new(5);
    let long_val = RetainedTextEditingValue {
        text: "123456789".into(),
        selection: RetainedTextSelection::collapsed(9),
        composing: None,
    };
    let limited = limit_formatter.format_edit_update(&old_val, &long_val);
    assert_eq!(limited.text, "12345");
}

#[test]
fn test_semantics_builder_surface() {
    let tapped = Rc::new(std::cell::Cell::new(false));
    let tapped_clone = tapped.clone();

    let widget: Widget = Semantics::new(Text::new("Submit"))
        .label("Submit button")
        .hint("Submits the form")
        .button(true)
        .enabled(true)
        .on_tap(move || tapped_clone.set(true))
        .into();

    let _ = widget;
}
