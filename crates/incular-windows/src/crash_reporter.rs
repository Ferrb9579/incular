use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crash_handler::{CrashEventResult, CrashHandler, ExceptionCode};
use minidump_writer::minidump_writer::MinidumpWriter;

static DUMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Installs the last-resort Windows exception handler for failures that occur
/// below Incular's guarded Rust frame pipeline. The returned handler must stay
/// alive for the complete native event loop.
pub(crate) fn install() -> Option<CrashHandler> {
    let directory = crash_report_directory();
    // SAFETY: The callback performs only bounded crash-report work and never
    // attempts to recover execution. `crash-handler` owns registration and
    // invokes it with a valid exception context for the callback duration.
    let event = unsafe {
        crash_handler::make_crash_event(move |context| {
            if context.exception_code != ExceptionCode::StackOverflow as i32 {
                return CrashEventResult::Handled(false);
            }
            let handled = write_minidump(&directory, context).is_some();
            CrashEventResult::Handled(handled)
        })
    };
    CrashHandler::attach(event).ok()
}

fn write_minidump(
    directory: &std::path::Path,
    context: &crash_handler::CrashContext,
) -> Option<PathBuf> {
    fs::create_dir_all(directory).ok()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = DUMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = directory.join(format!(
        "stack-overflow-{}-{timestamp}-{sequence}.dmp",
        std::process::id()
    ));
    let mut file = fs::File::create(&path).ok()?;
    MinidumpWriter::dump_crash_context(context, None, &mut file).ok()?;
    Some(path)
}

fn crash_report_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("INCULAR_CRASH_REPORT_DIR") {
        return PathBuf::from(path);
    }
    std::env::var_os("LOCALAPPDATA").map_or_else(
        || std::env::temp_dir().join("incular-crashes"),
        |path| PathBuf::from(path).join("Incular").join("CrashReports"),
    )
}
