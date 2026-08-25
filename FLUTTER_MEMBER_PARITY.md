# Incular Flutter Member Parity Report

> **Flutter Baseline**: `v3.47.0` (`stable` channel, Dart SDK `3.11.0`)
> **Snapshot Timestamp**: `2026-08-24T00:00:00Z`
> **Total Types Considered**: `333` (`74` deep member-audited types)
> **Total Audited Canonical Members**: `471`
> **Resolution Rate**: `100% (0 unresolved)`

> **Package boundary note (2026-08-25):** “Resolved” means every audited
> member has an explicit mapping or disposition; it does not claim that every
> Flutter implementation detail is identical. Core `widgets` primitives are
> kept in `incular-widgets`, while Material-only names (`TextField`,
> `TextFormField`, `SelectableText`, `SelectionArea`, `Autocomplete`, and the
> concrete button families) are exposed from `incular-material`. Incular
> extensions such as `VirtualList` remain explicitly classified as extensions.

## Executive Summary

Task 19 establishes API soundness, Flutter 3.47 stable baseline conformance, and a single unified canonical API graph.
Every canonical public type in Incular provides Flutter-equivalent semantics mapped to idiomatic Rust APIs, snake_case methods, SCREAMING_SNAKE_CASE constants, fluent builders, and independent bitflag `Invalidation` damage tracking.

## Unified API Graph & Dimensional Coverage

| Metric | Considered | Implemented / Resolved | Notes |
| :--- | :--- | :--- | :--- |
| **Type Decisions** | `333` | `333` | 100% explicit decisions across all Flutter core widgets and value types |
| **Member Parity** | `471` | `471` | 100% resolved to idiomatic Rust APIs |
| **Invalidation Model** | 6 Phases | 6 Independent Phases | Multi-dimensional `BUILD`, `LAYOUT`, `PAINT`, `COMPOSITE`, `SEMANTICS`, `HIT_TEST` bitflags |
| **Unresolved APIs** | 0 | 0 | 0 pending, 0 missing, 0 ambiguous |

## Category Breakdown

| Category | Members Audited | Canonical Types | Status |
| :--- | :--- | :--- | :--- |
| `animation` | 43 | 3 | **100% Resolved** |
| `core` | 6 | 2 | **100% Resolved** |
| `focus` | 12 | 2 | **100% Resolved** |
| `forms` | 20 | 5 | **100% Resolved** |
| `geometry` | 58 | 5 | **100% Resolved** |
| `gestures` | 46 | 14 | **100% Resolved** |
| `painting` | 77 | 14 | **100% Resolved** |
| `physics` | 18 | 8 | **100% Resolved** |
| `scrolling` | 76 | 12 | **100% Resolved** |
| `semantics` | 31 | 1 | **100% Resolved** |
| `text` | 84 | 12 | **100% Resolved** |

## Audited Canonical Member Inventory

### Subsystem: `animation`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Curve.apply` | `method` | `Curve::apply` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curve.reverse` | `method` | `Curve::reverse` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curve.then` | `method` | `Curve::then` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.ease` | `constant` | `Curves::EASE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeIn` | `constant` | `Curves::EASEIN` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInBack` | `constant` | `Curves::EASEINBACK` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInBounce` | `constant` | `Curves::EASEINBOUNCE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInCirc` | `constant` | `Curves::EASEINCIRC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInCubic` | `constant` | `Curves::EASEINCUBIC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInElastic` | `constant` | `Curves::EASEINELASTIC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInExpo` | `constant` | `Curves::EASEINEXPO` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOut` | `constant` | `Curves::EASEINOUT` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutBack` | `constant` | `Curves::EASEINOUTBACK` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutBounce` | `constant` | `Curves::EASEINOUTBOUNCE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutCirc` | `constant` | `Curves::EASEINOUTCIRC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutCubic` | `constant` | `Curves::EASEINOUTCUBIC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutElastic` | `constant` | `Curves::EASEINOUTELASTIC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutExpo` | `constant` | `Curves::EASEINOUTEXPO` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutQuad` | `constant` | `Curves::EASEINOUTQUAD` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutQuart` | `constant` | `Curves::EASEINOUTQUART` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutQuint` | `constant` | `Curves::EASEINOUTQUINT` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInOutSine` | `constant` | `Curves::EASEINOUTSINE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInQuad` | `constant` | `Curves::EASEINQUAD` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInQuart` | `constant` | `Curves::EASEINQUART` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInQuint` | `constant` | `Curves::EASEINQUINT` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeInSine` | `constant` | `Curves::EASEINSINE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOut` | `constant` | `Curves::EASEOUT` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutBack` | `constant` | `Curves::EASEOUTBACK` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutBounce` | `constant` | `Curves::EASEOUTBOUNCE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutCirc` | `constant` | `Curves::EASEOUTCIRC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutCubic` | `constant` | `Curves::EASEOUTCUBIC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutElastic` | `constant` | `Curves::EASEOUTELASTIC` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutExpo` | `constant` | `Curves::EASEOUTEXPO` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutQuad` | `constant` | `Curves::EASEOUTQUAD` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutQuart` | `constant` | `Curves::EASEOUTQUART` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutQuint` | `constant` | `Curves::EASEOUTQUINT` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.easeOutSine` | `constant` | `Curves::EASEOUTSINE` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.fastOutSlowIn` | `constant` | `Curves::FASTOUTSLOWIN` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Curves.linear` | `constant` | `Curves::LINEAR` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tween.begin` | `method` | `Tween::begin` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tween.end` | `method` | `Tween::end` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tween.lerp` | `method` | `Tween::lerp` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tween.transform` | `method` | `Tween::transform` | `incular-animation` | `implemented` | Direct idiomatic mapping |

### Subsystem: `core`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `ChangeImpact.Build` | `enum_variant` | `ChangeImpact::build` | `incular-core` | `implemented` | Incular damage invalidation domain |
| `ChangeImpact.Composite` | `enum_variant` | `ChangeImpact::composite` | `incular-core` | `implemented` | Incular damage invalidation domain |
| `ChangeImpact.Layout` | `enum_variant` | `ChangeImpact::layout` | `incular-core` | `implemented` | Incular damage invalidation domain |
| `ChangeImpact.None` | `enum_variant` | `ChangeImpact::none` | `incular-core` | `implemented` | Incular damage invalidation domain |
| `ChangeImpact.Paint` | `enum_variant` | `ChangeImpact::paint` | `incular-core` | `implemented` | Incular damage invalidation domain |
| `Lerp.lerp` | `method` | `Lerp::lerp` | `incular-core` | `implemented` | Authoritative linear interpolation contract |

### Subsystem: `focus`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `FocusNode.canRequestFocus` | `method` | `FocusNode::can_request_focus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.descendantsAreFocusable` | `method` | `FocusNode::descendants_are_focusable` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.descendantsAreTraversable` | `method` | `FocusNode::descendants_are_traversable` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.hasFocus` | `method` | `FocusNode::has_focus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.hasPrimaryFocus` | `method` | `FocusNode::has_primary_focus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.requestFocus` | `method` | `FocusNode::request_focus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.skipTraversal` | `method` | `FocusNode::skip_traversal` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusNode.unfocus` | `method` | `FocusNode::unfocus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusTraversalPolicy.findFirstFocus` | `method` | `FocusTraversalPolicy::find_first_focus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusTraversalPolicy.findLastFocus` | `method` | `FocusTraversalPolicy::find_last_focus` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusTraversalPolicy.next` | `method` | `FocusTraversalPolicy::next` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `FocusTraversalPolicy.previous` | `method` | `FocusTraversalPolicy::previous` | `incular-gestures` | `implemented` | Direct idiomatic mapping |

### Subsystem: `forms`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `FilteringTextInputFormatter.allow` | `method` | `FilteringTextInputFormatter::allow` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FilteringTextInputFormatter.deny` | `method` | `FilteringTextInputFormatter::deny` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FilteringTextInputFormatter.digitsOnly` | `method` | `FilteringTextInputFormatter::digits_only` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FilteringTextInputFormatter.formatEditUpdate` | `method` | `FilteringTextInputFormatter::format_edit_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FilteringTextInputFormatter.singleLineFormatter` | `method` | `FilteringTextInputFormatter::single_line_formatter` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FormFieldState.didChange` | `method` | `FormFieldState::did_change` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FormFieldState.errorText` | `method` | `FormFieldState::error_text` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FormFieldState.hasError` | `method` | `FormFieldState::has_error` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FormFieldState.isValid` | `method` | `FormFieldState::is_valid` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FormFieldState.reset` | `method` | `FormFieldState::reset` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `FormFieldState.value` | `method` | `FormFieldState::value` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GenericFormField.autovalidateMode` | `property` | `GenericFormField::autovalidate_mode` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GenericFormField.enabled` | `property` | `GenericFormField::enabled` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GenericFormField.initialValue` | `property` | `GenericFormField::initial_value` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GenericFormField.onSaved` | `property` | `GenericFormField::on_saved` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GenericFormField.restorationId` | `property` | `GenericFormField::restoration_id` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GenericFormField.validator` | `property` | `GenericFormField::validator` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `LengthLimitingTextInputFormatter.maxLength` | `property` | `LengthLimitingTextInputFormatter::max_length` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `LengthLimitingTextInputFormatter.maxLengthEnforcement` | `property` | `LengthLimitingTextInputFormatter::max_length_enforcement` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `TextInputFormatter.formatEditUpdate` | `method` | `TextInputFormatter::format_edit_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |

### Subsystem: `geometry`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Alignment.*` | `method` | `Alignment::*` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.+` | `method` | `Alignment::+` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.-` | `method` | `Alignment::-` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment./` | `method` | `Alignment::/` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.bottomCenter` | `method` | `Alignment::BOTTOM_CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.bottomLeft` | `method` | `Alignment::BOTTOM_LEFT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.bottomRight` | `method` | `Alignment::BOTTOM_RIGHT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.center` | `method` | `Alignment::CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.centerLeft` | `method` | `Alignment::CENTER_LEFT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.centerRight` | `method` | `Alignment::CENTER_RIGHT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.lerp` | `method` | `Alignment::lerp` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.resolve` | `method` | `Alignment::resolve` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.topCenter` | `method` | `Alignment::TOP_CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.topLeft` | `method` | `Alignment::TOP_LEFT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `Alignment.topRight` | `method` | `Alignment::TOP_RIGHT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.*` | `method` | `AlignmentDirectional::*` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.+` | `method` | `AlignmentDirectional::+` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.-` | `method` | `AlignmentDirectional::-` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional./` | `method` | `AlignmentDirectional::/` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.bottomCenter` | `method` | `AlignmentDirectional::BOTTOM_CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.bottomEnd` | `method` | `AlignmentDirectional::BOTTOM_END` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.bottomStart` | `method` | `AlignmentDirectional::BOTTOM_START` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.center` | `method` | `AlignmentDirectional::CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.centerEnd` | `method` | `AlignmentDirectional::CENTER_END` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.centerStart` | `method` | `AlignmentDirectional::CENTER_START` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.lerp` | `method` | `AlignmentDirectional::lerp` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.resolve` | `method` | `AlignmentDirectional::resolve` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.topCenter` | `method` | `AlignmentDirectional::TOP_CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.topEnd` | `method` | `AlignmentDirectional::TOP_END` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `AlignmentDirectional.topStart` | `method` | `AlignmentDirectional::TOP_START` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.*` | `method` | `EdgeInsets::*` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.+` | `method` | `EdgeInsets::+` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.-` | `method` | `EdgeInsets::-` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets./` | `method` | `EdgeInsets::/` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.all` | `method` | `EdgeInsets::all` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.lerp` | `method` | `EdgeInsets::lerp` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.only` | `method` | `EdgeInsets::only` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.resolve` | `method` | `EdgeInsets::resolve` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.symmetric` | `method` | `EdgeInsets::symmetric` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsets.zero` | `method` | `EdgeInsets::ZERO` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.*` | `method` | `EdgeInsetsDirectional::*` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.+` | `method` | `EdgeInsetsDirectional::+` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.-` | `method` | `EdgeInsetsDirectional::-` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional./` | `method` | `EdgeInsetsDirectional::/` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.all` | `method` | `EdgeInsetsDirectional::all` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.lerp` | `method` | `EdgeInsetsDirectional::lerp` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.only` | `method` | `EdgeInsetsDirectional::only` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.resolve` | `method` | `EdgeInsetsDirectional::resolve` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.symmetric` | `method` | `EdgeInsetsDirectional::symmetric` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `EdgeInsetsDirectional.zero` | `method` | `EdgeInsetsDirectional::zero` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.bottomRight` | `method` | `FractionalOffset::BOTTOM_RIGHT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.center` | `method` | `FractionalOffset::CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.dx` | `method` | `FractionalOffset::dx` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.dy` | `method` | `FractionalOffset::dy` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.lerp` | `method` | `FractionalOffset::lerp` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.topCenter` | `method` | `FractionalOffset::TOP_CENTER` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.topLeft` | `method` | `FractionalOffset::TOP_LEFT` | `incular-config` | `implemented` | Direct idiomatic mapping |
| `FractionalOffset.topRight` | `method` | `FractionalOffset::TOP_RIGHT` | `incular-config` | `implemented` | Direct idiomatic mapping |

### Subsystem: `gestures`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `DragDownDetails.globalPosition` | `property` | `DragDownDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `DragEndDetails.globalPosition` | `property` | `DragEndDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `DragStartDetails.globalPosition` | `property` | `DragStartDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `DragUpdateDetails.globalPosition` | `property` | `DragUpdateDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.behavior` | `method` | `GestureDetector::behavior` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.excludeFromSemantics` | `method` | `GestureDetector::exclude_from_semantics` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onDoubleTap` | `method` | `GestureDetector::on_double_tap` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onDoubleTapCancel` | `method` | `GestureDetector::on_double_tap_cancel` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onDoubleTapDown` | `method` | `GestureDetector::on_double_tap_down` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onHorizontalDragCancel` | `method` | `GestureDetector::on_horizontal_drag_cancel` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onHorizontalDragDown` | `method` | `GestureDetector::on_horizontal_drag_down` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onHorizontalDragEnd` | `method` | `GestureDetector::on_horizontal_drag_end` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onHorizontalDragStart` | `method` | `GestureDetector::on_horizontal_drag_start` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onHorizontalDragUpdate` | `method` | `GestureDetector::on_horizontal_drag_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onLongPress` | `method` | `GestureDetector::on_long_press` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onLongPressEnd` | `method` | `GestureDetector::on_long_press_end` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onLongPressMoveUpdate` | `method` | `GestureDetector::on_long_press_move_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onLongPressStart` | `method` | `GestureDetector::on_long_press_start` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onLongPressUp` | `method` | `GestureDetector::on_long_press_up` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onPanCancel` | `method` | `GestureDetector::on_pan_cancel` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onPanDown` | `method` | `GestureDetector::on_pan_down` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onPanEnd` | `method` | `GestureDetector::on_pan_end` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onPanStart` | `method` | `GestureDetector::on_pan_start` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onPanUpdate` | `method` | `GestureDetector::on_pan_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onScaleEnd` | `method` | `GestureDetector::on_scale_end` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onScaleStart` | `method` | `GestureDetector::on_scale_start` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onScaleUpdate` | `method` | `GestureDetector::on_scale_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onTap` | `method` | `GestureDetector::on_tap` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onTapCancel` | `method` | `GestureDetector::on_tap_cancel` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onTapDown` | `method` | `GestureDetector::on_tap_down` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onTapUp` | `method` | `GestureDetector::on_tap_up` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onVerticalDragCancel` | `method` | `GestureDetector::on_vertical_drag_cancel` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onVerticalDragDown` | `method` | `GestureDetector::on_vertical_drag_down` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onVerticalDragEnd` | `method` | `GestureDetector::on_vertical_drag_end` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onVerticalDragStart` | `method` | `GestureDetector::on_vertical_drag_start` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GestureDetector.onVerticalDragUpdate` | `method` | `GestureDetector::on_vertical_drag_update` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `LongPressEndDetails.globalPosition` | `property` | `LongPressEndDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `LongPressMoveUpdateDetails.globalPosition` | `property` | `LongPressMoveUpdateDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `LongPressStartDetails.globalPosition` | `property` | `LongPressStartDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `ScaleEndDetails.globalPosition` | `property` | `ScaleEndDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `ScaleStartDetails.globalPosition` | `property` | `ScaleStartDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `ScaleUpdateDetails.globalPosition` | `property` | `ScaleUpdateDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `TapDownDetails.globalPosition` | `property` | `TapDownDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `TapUpDetails.globalPosition` | `property` | `TapUpDetails::global_position` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `Velocity.ZERO` | `property` | `Velocity::zero` | `incular-gestures` | `implemented` | Direct idiomatic mapping |
| `Velocity.pixelsPerSecond` | `property` | `Velocity::pixelspersecond` | `incular-gestures` | `implemented` | Direct idiomatic mapping |

### Subsystem: `painting`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `BlurStyle.inner` | `enum_variant` | `BlurStyle::inner` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BlurStyle.normal` | `enum_variant` | `BlurStyle::normal` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BlurStyle.outer` | `enum_variant` | `BlurStyle::outer` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BlurStyle.solid` | `enum_variant` | `BlurStyle::solid` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Border.all` | `method` | `Border::all` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Border.dimensions` | `method` | `Border::dimensions` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Border.fromBorderSide` | `method` | `Border::from_border_side` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Border.lerp` | `method` | `Border::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Border.only` | `method` | `Border::only` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Border.symmetric` | `method` | `Border::symmetric` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.bottom` | `method` | `BorderDirectional::bottom` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.end` | `method` | `BorderDirectional::end` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.lerp` | `method` | `BorderDirectional::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.only` | `method` | `BorderDirectional::only` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.resolve` | `method` | `BorderDirectional::resolve` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.start` | `method` | `BorderDirectional::start` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderDirectional.top` | `method` | `BorderDirectional::top` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.all` | `method` | `BorderRadius::all` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.circular` | `method` | `BorderRadius::circular` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.horizontal` | `method` | `BorderRadius::horizontal` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.lerp` | `method` | `BorderRadius::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.only` | `method` | `BorderRadius::only` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.resolve` | `method` | `BorderRadius::resolve` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.toCornerRadii` | `method` | `BorderRadius::to_corner_radii` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.vertical` | `method` | `BorderRadius::vertical` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadius.zero` | `method` | `BorderRadius::zero` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.all` | `method` | `BorderRadiusDirectional::all` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.circular` | `method` | `BorderRadiusDirectional::circular` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.horizontal` | `method` | `BorderRadiusDirectional::horizontal` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.lerp` | `method` | `BorderRadiusDirectional::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.only` | `method` | `BorderRadiusDirectional::only` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.resolve` | `method` | `BorderRadiusDirectional::resolve` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.vertical` | `method` | `BorderRadiusDirectional::vertical` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderRadiusDirectional.zero` | `method` | `BorderRadiusDirectional::zero` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderSide.color` | `property` | `BorderSide::color` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderSide.lerp` | `property` | `BorderSide::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderSide.none` | `property` | `BorderSide::NONE` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderSide.strokeAlign` | `property` | `BorderSide::stroke_align` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderSide.style` | `property` | `BorderSide::style` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderSide.width` | `property` | `BorderSide::width` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderStyle.none` | `enum_variant` | `BorderStyle::none` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BorderStyle.solid` | `enum_variant` | `BorderStyle::solid` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.backgroundBlendMode` | `property` | `BoxDecoration::background_blend_mode` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.border` | `property` | `BoxDecoration::border` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.borderRadius` | `property` | `BoxDecoration::border_radius` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.boxShadow` | `property` | `BoxDecoration::box_shadow` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.color` | `property` | `BoxDecoration::color` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.copyWith` | `method` | `BoxDecoration::with_*` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.gradient` | `property` | `BoxDecoration::gradient` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.image` | `property` | `BoxDecoration::image` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.isValid` | `method` | `BoxDecoration::is_valid` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.lerp` | `method` | `BoxDecoration::lerp::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxDecoration.shape` | `property` | `BoxDecoration::shape` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShadow.blurRadius` | `property` | `BoxShadow::blur_radius` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShadow.blurStyle` | `property` | `BoxShadow::blur_style` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShadow.color` | `property` | `BoxShadow::color` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShadow.lerp` | `property` | `BoxShadow::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShadow.offset` | `property` | `BoxShadow::offset` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShadow.spreadRadius` | `property` | `BoxShadow::spread_radius` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShape.circle` | `enum_variant` | `BoxShape::circle` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `BoxShape.rectangle` | `enum_variant` | `BoxShape::rectangle` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `CustomPaint.painter` | `property` | `CustomPaint::from_painter` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `CustomPaint.size` | `property` | `CustomPaint::size` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `CustomPainter.paint` | `method` | `CustomPainter::paint` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `CustomPainter.shouldRepaint` | `method` | `CustomPainter::should_repaint` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.alignment` | `property` | `DecorationImage::alignment` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.centerSlice` | `property` | `DecorationImage::centerslice` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.fit` | `property` | `DecorationImage::fit` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.image` | `property` | `DecorationImage::image` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.matchTextDirection` | `property` | `DecorationImage::matchtextdirection` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.opacity` | `property` | `DecorationImage::opacity` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.repeat` | `property` | `DecorationImage::repeat` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `DecorationImage.scale` | `property` | `DecorationImage::scale` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Radius.circular` | `method` | `Radius::circular` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Radius.elliptical` | `method` | `Radius::elliptical` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Radius.lerp` | `method` | `Radius::lerp` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Radius.zero` | `method` | `Radius::ZERO` | `incular-widgets` | `implemented` | Direct idiomatic mapping |

### Subsystem: `physics`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `BoundedFrictionSimulation.new` | `constructor` | `BoundedFrictionSimulation::new` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `FrictionSimulation.new` | `constructor` | `FrictionSimulation::new` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `GravitySimulation.new` | `constructor` | `GravitySimulation::new` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `ScrollSpringSimulation.new` | `constructor` | `ScrollSpringSimulation::new` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Simulation.isDone` | `method` | `Simulation::is_done` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Simulation.position` | `method` | `Simulation::position` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Simulation.velocity` | `method` | `Simulation::velocity` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringDescription.damping` | `method` | `SpringDescription::damping` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringDescription.mass` | `method` | `SpringDescription::mass` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringDescription.springType` | `method` | `SpringDescription::spring_type` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringDescription.stiffness` | `method` | `SpringDescription::stiffness` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringDescription.withDampingRatio` | `method` | `SpringDescription::with_damping_ratio` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringDescription.withDurationAndBounce` | `method` | `SpringDescription::with_duration_and_bounce` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `SpringSimulation.new` | `constructor` | `SpringSimulation::new` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tolerance.DEFAULT` | `property` | `Tolerance::default` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tolerance.distance` | `property` | `Tolerance::distance` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tolerance.time` | `property` | `Tolerance::time` | `incular-animation` | `implemented` | Direct idiomatic mapping |
| `Tolerance.velocity` | `property` | `Tolerance::velocity` | `incular-animation` | `implemented` | Direct idiomatic mapping |

### Subsystem: `scrolling`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `GridDelegate.fixedCrossAxisCount` | `enum_variant` | `GridDelegate::fixed_cross_axis_count` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridDelegate.maxCrossAxisExtent` | `enum_variant` | `GridDelegate::max_cross_axis_extent` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridView.builder` | `method` | `GridView::builder` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridView.controller` | `method` | `GridView::controller` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridView.count` | `method` | `GridView::count` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridView.padding` | `method` | `GridView::padding` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridView.reverse` | `method` | `GridView::reverse` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `GridView.scrollDirection` | `method` | `GridView::scroll_direction` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ItemExtentStrategy.builder` | `enum_variant` | `ItemExtentStrategy::builder` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ItemExtentStrategy.fixed` | `enum_variant` | `ItemExtentStrategy::fixed` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ItemExtentStrategy.measured` | `enum_variant` | `ItemExtentStrategy::measured` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ItemExtentStrategy.prototype` | `enum_variant` | `ItemExtentStrategy::prototype` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `KeepAlivePolicy.automatic` | `enum_variant` | `KeepAlivePolicy::automatic` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `KeepAlivePolicy.disabled` | `enum_variant` | `KeepAlivePolicy::disabled` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `KeepAlivePolicy.manual` | `enum_variant` | `KeepAlivePolicy::manual` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.builder` | `method` | `ListView::builder` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.itemExtent` | `method` | `ListView::item_extent` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.keepAlivePolicy` | `method` | `ListView::keep_alive_policy` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.primary` | `method` | `ListView::primary` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.prototypeItem` | `method` | `ListView::prototype_item` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.repaintBoundaryPolicy` | `method` | `ListView::repaint_boundary_policy` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.restorationId` | `method` | `ListView::restoration_id` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.semanticIndexPolicy` | `method` | `ListView::semantic_index_policy` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.separated` | `method` | `ListView::separated` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ListView.shrinkWrap` | `method` | `ListView::shrink_wrap` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `PageView.builder` | `method` | `PageView::builder` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `PageView.controller` | `method` | `PageView::controller` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `PageView.new` | `method` | `PageView::new` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `PageView.pageSnapping` | `method` | `PageView::page_snapping` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `PageView.reverse` | `method` | `PageView::reverse` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `PageView.scrollDirection` | `method` | `PageView::scroll_direction` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `RepaintBoundaryPolicy.automatic` | `enum_variant` | `RepaintBoundaryPolicy::automatic` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `RepaintBoundaryPolicy.disabled` | `enum_variant` | `RepaintBoundaryPolicy::disabled` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `RepaintBoundaryPolicy.manual` | `enum_variant` | `RepaintBoundaryPolicy::manual` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `ScrollCacheExtent.pixels` | `method` | `ScrollCacheExtent::pixels` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollCacheExtent.toPixels` | `method` | `ScrollCacheExtent::to_pixels` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollCacheExtent.viewport` | `method` | `ScrollCacheExtent::viewport` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.atEdge` | `method` | `ScrollMetrics::at_edge` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.axis` | `method` | `ScrollMetrics::axis` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.axisDirection` | `method` | `ScrollMetrics::axis_direction` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.extentAfter` | `method` | `ScrollMetrics::extent_after` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.extentBefore` | `method` | `ScrollMetrics::extent_before` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.extentInside` | `method` | `ScrollMetrics::extent_inside` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.extentTotal` | `method` | `ScrollMetrics::extent_total` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.maxScrollExtent` | `method` | `ScrollMetrics::max_scroll_extent` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.minScrollExtent` | `method` | `ScrollMetrics::min_scroll_extent` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.outOfRange` | `method` | `ScrollMetrics::out_of_range` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.pixels` | `method` | `ScrollMetrics::pixels` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollMetrics.viewportDimension` | `method` | `ScrollMetrics::viewport_dimension` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.alwaysScrollable` | `method` | `ScrollPhysics::always_scrollable` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.applyDelta` | `method` | `ScrollPhysics::apply_delta` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.bouncing` | `method` | `ScrollPhysics::bouncing` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.clamping` | `method` | `ScrollPhysics::clamping` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.dragStartDistanceMotionThreshold` | `method` | `ScrollPhysics::drag_start_distance_motion_threshold` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.fixedExtentSnapping` | `method` | `ScrollPhysics::fixed_extent_snapping` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.maxFlingVelocity` | `method` | `ScrollPhysics::max_fling_velocity` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.minFlingVelocity` | `method` | `ScrollPhysics::min_fling_velocity` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.neverScrollable` | `method` | `ScrollPhysics::never_scrollable` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.pageSnapping` | `method` | `ScrollPhysics::page_snapping` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.parent` | `method` | `ScrollPhysics::parent` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollPhysics.rangeMaintaining` | `method` | `ScrollPhysics::range_maintaining` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.clipBehavior` | `property` | `ScrollViewConfig::clip_behavior` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.direction` | `property` | `ScrollViewConfig::direction` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.dragStartBehavior` | `property` | `ScrollViewConfig::drag_start_behavior` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.keyboardDismissBehavior` | `property` | `ScrollViewConfig::keyboard_dismiss_behavior` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.padding` | `property` | `ScrollViewConfig::padding` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.physics` | `property` | `ScrollViewConfig::physics` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.primary` | `property` | `ScrollViewConfig::primary` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.restorationId` | `property` | `ScrollViewConfig::restoration_id` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.reverse` | `property` | `ScrollViewConfig::reverse` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.scrollCacheExtent` | `property` | `ScrollViewConfig::scroll_cache_extent` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.semanticChildCount` | `property` | `ScrollViewConfig::semantic_child_count` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `ScrollViewConfig.shrinkWrap` | `property` | `ScrollViewConfig::shrink_wrap` | `incular-scroll` | `implemented` | Direct idiomatic mapping |
| `SemanticIndexPolicy.automatic` | `enum_variant` | `SemanticIndexPolicy::automatic` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `SemanticIndexPolicy.disabled` | `enum_variant` | `SemanticIndexPolicy::disabled` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `SemanticIndexPolicy.manual` | `enum_variant` | `SemanticIndexPolicy::manual` | `incular-widgets` | `implemented` | Direct idiomatic mapping |

### Subsystem: `semantics`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Semantics.button` | `method` | `Semantics::button` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.checked` | `method` | `Semantics::checked` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.decreasedValue` | `method` | `Semantics::decreased_value` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.enabled` | `method` | `Semantics::enabled` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.focused` | `method` | `Semantics::focused` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.header` | `method` | `Semantics::header` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.hint` | `method` | `Semantics::hint` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.image` | `method` | `Semantics::image` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.increasedValue` | `method` | `Semantics::increased_value` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.label` | `method` | `Semantics::label` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.link` | `method` | `Semantics::link` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.multiline` | `method` | `Semantics::multiline` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.obscured` | `method` | `Semantics::obscured` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onCopy` | `method` | `Semantics::on_copy` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onCut` | `method` | `Semantics::on_cut` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onDecrease` | `method` | `Semantics::on_decrease` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onDismiss` | `method` | `Semantics::on_dismiss` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onIncrease` | `method` | `Semantics::on_increase` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onPaste` | `method` | `Semantics::on_paste` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onScrollDown` | `method` | `Semantics::on_scroll_down` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onScrollLeft` | `method` | `Semantics::on_scroll_left` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onScrollRight` | `method` | `Semantics::on_scroll_right` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onScrollUp` | `method` | `Semantics::on_scroll_up` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.onTap` | `method` | `Semantics::on_tap` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.readOnly` | `method` | `Semantics::read_only` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.selected` | `method` | `Semantics::selected` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.slider` | `method` | `Semantics::slider` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.textDirection` | `method` | `Semantics::text_direction` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.toggled` | `method` | `Semantics::toggled` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.tooltip` | `method` | `Semantics::tooltip` | `incular-widgets` | `implemented` | Direct idiomatic mapping |
| `Semantics.value` | `method` | `Semantics::value` | `incular-widgets` | `implemented` | Direct idiomatic mapping |

### Subsystem: `text`

| Flutter Type & Member | Member Kind | Incular Type & Member | In Incular Crate | Status | Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `FontFeature.alternative` | `constructor` | `FontFeature::alternative` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontFeature.contextualAlternates` | `constructor` | `FontFeature::contextual_alternates` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontFeature.disable` | `constructor` | `FontFeature::disable` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontFeature.enable` | `constructor` | `FontFeature::enable` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontFeature.proportionalFigures` | `constructor` | `FontFeature::proportionalfigures` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontFeature.slashedZero` | `constructor` | `FontFeature::slashedzero` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontFeature.tabularFigures` | `constructor` | `FontFeature::tabular_figures` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontVariation.italic` | `constructor` | `FontVariation::italic` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontVariation.opticalSize` | `constructor` | `FontVariation::optical_size` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontVariation.slant` | `constructor` | `FontVariation::slant` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontVariation.weight` | `constructor` | `FontVariation::weight` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontVariation.width` | `constructor` | `FontVariation::width` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.bold` | `constant` | `FontWeight::BOLD` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.lerp` | `method` | `FontWeight::lerp::lerp` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.normal` | `constant` | `FontWeight::NORMAL` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w100` | `constant` | `FontWeight::W100` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w200` | `constant` | `FontWeight::W200` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w300` | `constant` | `FontWeight::W300` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w400` | `constant` | `FontWeight::W400` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w500` | `constant` | `FontWeight::W500` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w600` | `constant` | `FontWeight::W600` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w700` | `constant` | `FontWeight::W700` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w800` | `constant` | `FontWeight::W800` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `FontWeight.w900` | `constant` | `FontWeight::W900` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.debugLabel` | `property` | `StrutStyle::debuglabel` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.fontFamily` | `property` | `StrutStyle::fontfamily` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.fontSize` | `property` | `StrutStyle::fontsize` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.fontStyle` | `property` | `StrutStyle::fontstyle` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.fontWeight` | `property` | `StrutStyle::fontweight` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.forceStrutHeight` | `property` | `StrutStyle::forcestrutheight` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.height` | `property` | `StrutStyle::height` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `StrutStyle.leading` | `property` | `StrutStyle::leading` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextBaseline.alphabetic` | `enum_variant` | `TextBaseline::alphabetic` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextBaseline.ideographic` | `enum_variant` | `TextBaseline::ideographic` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecoration.combine` | `method` | `TextDecoration::combine` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecoration.contains` | `method` | `TextDecoration::contains` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecoration.lineThrough` | `method` | `TextDecoration::LINE_THROUGH` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecoration.none` | `method` | `TextDecoration::NONE` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecoration.overline` | `method` | `TextDecoration::OVERLINE` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecoration.underline` | `method` | `TextDecoration::UNDERLINE` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecorationStyle.dashed` | `enum_variant` | `TextDecorationStyle::dashed` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecorationStyle.dotted` | `enum_variant` | `TextDecorationStyle::dotted` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecorationStyle.double` | `enum_variant` | `TextDecorationStyle::double` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecorationStyle.solid` | `enum_variant` | `TextDecorationStyle::solid` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextDecorationStyle.wavy` | `enum_variant` | `TextDecorationStyle::wavy` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextHeightBehavior.applyHeightToFirstAscent` | `property` | `TextHeightBehavior::applyheighttofirstascent` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextHeightBehavior.applyHeightToLastDescent` | `property` | `TextHeightBehavior::applyheighttolastdescent` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextHeightBehavior.leadingDistribution` | `property` | `TextHeightBehavior::leadingdistribution` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextLeadingDistribution.even` | `enum_variant` | `TextLeadingDistribution::even` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextLeadingDistribution.proportional` | `enum_variant` | `TextLeadingDistribution::proportional` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextShadow.blurRadius` | `property` | `TextShadow::blurradius` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextShadow.color` | `property` | `TextShadow::color` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextShadow.lerp` | `property` | `TextShadow::lerp` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextShadow.offset` | `property` | `TextShadow::offset` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.apply` | `method` | `TextStyle::apply` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.background` | `property` | `TextStyle::background` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.backgroundColor` | `property` | `TextStyle::background_color` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.changeImpact` | `method` | `TextStyle::change_impact` | `incular-text` | `implemented` | Calculates retained invalidation damage (Layout vs Paint vs None) |
| `TextStyle.color` | `property` | `TextStyle::color` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.copyWith` | `method` | `TextStyle::with_*` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.debugLabel` | `property` | `TextStyle::debug_label` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.decoration` | `property` | `TextStyle::decoration` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.decorationColor` | `property` | `TextStyle::decoration_color` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.decorationStyle` | `property` | `TextStyle::decoration_style` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.decorationThickness` | `property` | `TextStyle::decoration_thickness` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.fontFeatures` | `property` | `TextStyle::font_features` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.fontSize` | `property` | `TextStyle::font_size` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.fontStyle` | `property` | `TextStyle::font_style` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.fontVariations` | `property` | `TextStyle::font_variations` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.fontWeight` | `property` | `TextStyle::font_weight` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.foreground` | `property` | `TextStyle::foreground` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.height` | `property` | `TextStyle::line_height` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.inherit` | `property` | `TextStyle::inherit` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.leadingDistribution` | `property` | `TextStyle::leading_distribution` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.lerp` | `method` | `TextStyle::lerp::lerp` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.letterSpacing` | `property` | `TextStyle::letter_spacing` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.locale` | `property` | `TextStyle::locale` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.merge` | `method` | `TextStyle::merge` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.overflow` | `property` | `TextStyle::overflow` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.shadows` | `property` | `TextStyle::shadows` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.textBaseline` | `property` | `TextStyle::text_baseline` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextStyle.wordSpacing` | `property` | `TextStyle::word_spacing` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextWidthBasis.longestLine` | `enum_variant` | `TextWidthBasis::longest_line` | `incular-text` | `implemented` | Direct idiomatic mapping |
| `TextWidthBasis.parent` | `enum_variant` | `TextWidthBasis::parent` | `incular-text` | `implemented` | Direct idiomatic mapping |
