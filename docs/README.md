# Documentation

Architecture notes, design decisions, and contributor-facing documentation
will be collected here.
# Architecture notes

## Phase 6 input slice

Focus is held by `Runtime` as an `ElementId`; `Tab` traverses mounted focusable
nodes through the active widget-order, reading-order, or explicit-order policy,
and focused text fields paint their own outline. The platform layer translates
winit keyboard, text and IME events into core types, leaving widgets independent
of winit. IME preedit is visual-only and is never appended to
`TextEditingController::text()` until a commit arrives. Native text-input
clients receive stable selection, composing, and caret-rectangle updates, and
soft-keyboard actions are routed back to the focused editor.

Text editing is controller-based. `TextField` is single-line and submits on
Enter. `TextArea` is bounded and wrapped: Enter/Shift+Enter add hard breaks,
ArrowLeft/Right follow shaped visual caret stops, including mixed-direction
paragraphs; ArrowUp/Down and Home/End use shaped line geometry, and its local
viewport keeps the caret visible. Backspace/Delete use extended grapheme
boundaries; selection and preedit ranges are valid UTF-8 byte ranges. Committed
edits are retained by the runtime's bounded undo/redo history, while IME
preedit remains separate until commit.

All vertical viewports share `ScrollController` extents. Incular draws an
always-visible overlay scrollbar when content overflows; its thumb captures
drags and track clicks page by a viewport. Winit `LineDelta` and fractional
`PixelDelta` both use positive-Y-to-increasing-offset with no framework-level
natural-scroll inversion. Accessibility projections publish richer roles,
state, editable text, selection, numeric actions, incremental mobile updates,
and generation-safe native IDs; Android and iOS consume the same
platform-neutral mobile projection contract.
