# DevTools and diagnostics privacy

The `devtools` feature is opt-in. Build the checkout's companion application:

```text
cargo build -p incular-devtools-ui
cargo run -p incular --features devtools --example counter -- --devtools
```

The UI is a workspace tool with `publish = false`. `INCULAR_DEVTOOLS_UI` can name
its executable. `INCULAR_DEVTOOLS` requests instrumentation when the feature is
enabled. Shipped applications should make diagnostics activation deliberate.

The agent listens on an ephemeral IPv4 loopback port and requires a random
per-session token in the Hello exchange. Handshake timeouts, frame/message limits
and bounded queues constrain input. The HTTP upgrade also times out, cancels
on shutdown, and rejects browser-origin requests before authentication. Do not
expose/proxy the port to a network.

Discovery records in the user's Incular DevTools data directory contain a
process ID, application name, port and token. Exclusive randomized temporary
files are privately created on Unix and inherit the per-user directory ACL on
Windows, then are atomically persisted. Teardown removes only the matching
record. Processes with the same user's access are not an isolation boundary.
Keep the data directory private; copied discovery files contain credentials.

Debug payloads can expose widget labels/text, structure, paths, screenshots and
property values. Review logs/recordings before sharing. Applications own their
redaction and retention policy. No hosted telemetry upload is configured by the
framework's local transport.

The Windows desktop runner installs a native crash handler while active. Native
stack-overflow dumps may be written to `%LOCALAPPDATA%\Incular\CrashReports`, or
`INCULAR_CRASH_REPORT_DIR`. Dumps may contain memory, paths and sensitive data.
Restrict access, review before sharing, and apply an application retention policy.
