# Plan 09 - Native File Dialogs and Document/File Integration

## Goal

Add professional OS-native file/folder selection and save workflows, then connect selected/opened documents cleanly to application code.

This is distinct from Incular-rendered `Dialog` widgets.

## Architecture

### Native dialog service

Add an application/window-scoped service with typed async APIs for:

- open one file;
- open multiple files;
- save file;
- select folder;
- filters/content types;
- suggested file name/location where supported.

Dialog results should use portable path/document descriptors, not native dialog objects.

Use a mature Rust crate if it meets platform-quality, async/lifecycle, filter, ownership, and packaging requirements. If a crate falls short on one OS, keep the portable service and implement that OS adapter natively rather than lowering the API permanently.

### Parent ownership

Native dialogs must be correctly parented/modally associated with the requesting Incular window where the OS supports it.

Do not block the retained UI/event loop waiting synchronously for dialog completion.

### Document integration

Define a small application-level file/document activation value reused by Plan 11 for OS "open file" activation. Do not build a document-model framework unless necessary; Incular should transport file intent, not own user document persistence policy.

## Crate ownership

- `incular-platform`: portable dialog request/result/filter contracts.
- `incular-runtime`: async service handles and window association.
- OS crates or a validated shared desktop adapter: native dialog execution.
- no native-dialog code in Material widgets.

## Hard invariants

1. Native dialogs never block the UI event loop.
2. Closing the parent/requesting app resolves/cancels outstanding dialog requests deterministically.
3. Filters are semantic and do not depend on display labels.
4. Native handles do not escape the backend.
5. Unsupported options are reported, not silently ignored when semantically important.

## Tests

- Request/result/cancel lifecycle through memory adapter.
- Multiple concurrent dialog requests follow documented serialization policy.
- Parent close cancels request exactly once.
- File filters normalize extensions/content types.
- Save dialog preserves selected destination exactly.
- Live smoke tests on each desktop OS.

## Acceptance criteria

Application code can implement File -> Open/Save/Select Folder using only Incular APIs with native OS dialogs.
