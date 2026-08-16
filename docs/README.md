# Documentation

Architecture notes, design decisions, and contributor-facing documentation
will be collected here.
# Architecture notes

## Phase 6 input slice

Focus is held by `Runtime` as an `ElementId`; `Tab` traverses mounted buttons
and text fields in tree order, and focused text fields paint their own outline.
The platform layer translates winit keyboard, text and IME events into core
types, leaving widgets independent of winit. IME preedit is visual-only and is
never appended to `TextEditingController::text()` until a commit arrives.

Text editing is controller-based. `TextField` is single-line and submits on
Enter. `TextArea` is bounded and wrapped: Enter/Shift+Enter add hard breaks,
ArrowUp/Down and Home/End use shaped line geometry, and its local viewport
keeps the caret visible. Backspace/Delete use extended grapheme boundaries;
selection and preedit ranges are valid UTF-8 byte ranges. IME preedit remains
separate until commit.

All vertical viewports share `ScrollController` extents. Incular draws an
always-visible overlay scrollbar when content overflows; its thumb captures
drags and track clicks page by a viewport. Winit `LineDelta` and fractional
`PixelDelta` both use positive-Y-to-increasing-offset with no framework-level
natural-scroll inversion. Known gaps: bidi visual navigation, word movement,
accessibility text semantics, and non-Linux native clipboard backends.
