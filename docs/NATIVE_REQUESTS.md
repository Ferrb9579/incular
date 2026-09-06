# Native request ownership

Runtime native services share private `RequestChannel`, `RequestReceiver` and
`RequestRegistry` mechanisms. Public IDs, operations, errors and resource handles
remain domain-specific. No new public generic request API is required.

## Admission and completion

Window operations, file dialogs and shortcuts use one admission gate for enqueue
versus stop. An accepted request is included in the host's shutdown drain; later
requests receive their existing typed stopped error. Wake callbacks and reply
destructors run outside admission and wake locks, including failed sends.
Application shell state is UI-thread owned and serializes admission and stop
through its existing service owner, using the same pending registry.

| Registry phase | Meaning | Allowed exit |
| --- | --- | --- |
| Queued | Accepted, but not yet handed to a host | Dispatch, cancellation, owner close or shutdown |
| Dispatched | Host took the operation; this is not OS acknowledgement | Completion, abandonment, owner close or shutdown |
| Abandoned | Delivery detached while the domain still needs native completion | Native completion, owner close or shutdown |

Terminal requests are removed, rather than retained as tombstones. Completion
must match a dispatched request and, where relevant, its window generation.
Early, wrong-owner, duplicate and late callbacks cannot consume another reply.
Caller reply inspection returns a terminal result once; a future must not be
polled again after completion. Shutdown resolves pending replies even if the
`Application` value itself remains alive, including application-shell work.

## Cancellation and native resources

| Service | Before host handoff | After host handoff |
| --- | --- | --- |
| Window operation | Dropping the future removes delivery bookkeeping; the command may still execute | Delivery is detached; a native side effect is not undone |
| File dialog | Dropping the future removes queued presentation and advances that owner's FIFO | Delivery is detached; the modal slot stays occupied until completion or owner close |
| Shortcut registration | Dropping the future suppresses registration | Successful abandoned registration requires immediate host rollback; an unread successful reply owns a registration lease whose drop queues unregister |
| Tray / notification | Dropping the handle cancels creation and queued updates | Dropping the handle retires its generation and queues cleanup when supported; failed creation suppresses queued cleanup |

Dialogs serialize per owner, while different windows may run dialogs concurrently.
Ordinary window setters remain enqueue operations. Shortcut unregistration still
executes if its future is dropped, because it releases an owned native resource.
A failed tray/notification creation rejects further native updates and does not
require removal. Cancelled queued shell operations report `StaleResource` through
the existing completion stream; shutdown reports `ApplicationStopped`.

Native hosts remain responsible for their OS resource stores and teardown on host
exit. Runtime cancellation cannot retract an already-dispatched side effect.
Notification platforms without dismissal support retire runtime identity without
promising to remove an OS-visible notification. Operations already handed to a
host cannot be withdrawn from that host's batch; completion and cleanup must still
follow the typed backend contract.

## Evidence

`crates/incular-runtime/tests/native_service_lifecycle.rs` applies one lifecycle
suite to all four services: success, backend failure, completion once, shutdown
and late completion. It also covers dropped queued and unread successful shortcut
requests and early window completion. `native_request_protocol.rs` exercises the
shared admission race, reentrant wake/abandonment, failed-send teardown and registry
owner validation. Existing `platform_operations.rs` and `file_dialogs.rs` cover
concurrent windows, generation reuse, parent close, detached delivery and dialog
FIFO. `application_shell.rs` covers generational resources and cleanup before
creation, after dispatch and after creation failure. Tests use deterministic
adapters; they do not establish new native-platform capability claims.
