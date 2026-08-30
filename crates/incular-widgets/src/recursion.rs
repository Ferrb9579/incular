use std::{
    backtrace::Backtrace,
    cell::{Cell, RefCell},
    fmt, fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use incular_config::Constraints;

use crate::tree::{ElementId, RenderObjectId};

const DEFAULT_RECURSION_LIMIT: usize = 256;
const MIN_RECURSION_LIMIT: usize = 16;
const MAX_RECURSION_LIMIT: usize = 4_096;

/// A retained frame-pipeline phase protected by Incular's recursion guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FramePhase {
    Build,
    Layout,
    Paint,
    Semantics,
    Compositor,
}

impl fmt::Display for FramePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Build => "Build",
            Self::Layout => "Layout",
            Self::Paint => "Paint",
            Self::Semantics => "Semantics",
            Self::Compositor => "Compositor",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DiagnosticNodeId {
    Element(ElementId),
    Render(RenderObjectId),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DiagnosticNode {
    pub phase: FramePhase,
    pub id: Option<DiagnosticNodeId>,
    pub kind: &'static str,
    pub constraints: Option<Constraints>,
}

impl DiagnosticNode {
    pub(crate) const fn phase_root(phase: FramePhase) -> Self {
        Self {
            phase,
            id: None,
            kind: "WidgetTree",
            constraints: None,
        }
    }
}

/// Structured snapshot persisted when Incular catches render recursion before
/// the process exhausts its native stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecursionReport {
    pub phase: FramePhase,
    pub reason: String,
    pub reentered: bool,
    pub trigger: Option<String>,
    pub repeated_node: String,
    pub constraints: Option<String>,
    pub widget_path: Vec<String>,
    pub last_external_call: Option<String>,
    pub backtrace: String,
    pub log_path: Option<PathBuf>,
}

impl RecursionReport {
    fn render(&self, include_log_path: bool) -> String {
        let mut output = format!(
            "Incular detected unsafe {} nesting\n\nTrigger: {}\nFrame phase: {}\nReason: {}\n{}: {}",
            self.phase.to_string().to_lowercase(),
            self.trigger.as_deref().unwrap_or("unknown"),
            self.phase,
            self.reason,
            if self.reentered {
                "Repeated node"
            } else {
                "Deepest node"
            },
            self.repeated_node,
        );
        if let Some(constraints) = &self.constraints {
            output.push_str("\nConstraints: ");
            output.push_str(constraints);
        }
        output.push_str("\n\nWidget path:\n");
        for (depth, node) in self.widget_path.iter().enumerate() {
            output.push_str(&"  ".repeat(depth));
            output.push_str(if depth == 0 { "" } else { "└─ " });
            output.push_str(node);
            if self.reentered && depth + 1 == self.widget_path.len() {
                output.push_str("  <-- entered twice");
            }
            output.push('\n');
        }
        output.push_str("\nLast external call: ");
        output.push_str(self.last_external_call.as_deref().unwrap_or("none"));
        if include_log_path && let Some(path) = &self.log_path {
            output.push_str("\nCrash report: ");
            output.push_str(&path.display().to_string());
        }
        output.push_str("\n\nBacktrace:\n");
        output.push_str(&self.backtrace);
        output
    }
}

impl fmt::Display for RecursionReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render(true))
    }
}

struct RecursionState {
    active: Vec<DiagnosticNode>,
    trigger: Option<String>,
    external_calls: Vec<String>,
    last_report: Option<RecursionReport>,
    report_directory: PathBuf,
    limit: usize,
}

impl RecursionState {
    fn new() -> Self {
        Self {
            active: Vec::with_capacity(64),
            trigger: None,
            external_calls: Vec::with_capacity(4),
            last_report: None,
            report_directory: default_report_directory(),
            limit: configured_recursion_limit(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RecursionDiagnostics {
    inner: Rc<RecursionDiagnosticsInner>,
}

struct RecursionDiagnosticsInner {
    depth: Cell<usize>,
    state: RefCell<RecursionState>,
}

impl RecursionDiagnostics {
    pub(crate) fn new() -> Self {
        Self {
            inner: Rc::new(RecursionDiagnosticsInner {
                depth: Cell::new(0),
                state: RefCell::new(RecursionState::new()),
            }),
        }
    }

    pub(crate) fn enter(&self, node: DiagnosticNode) -> ActivePhaseGuard {
        let next_depth = self.inner.depth.get().saturating_add(1);
        let full_tracking = cfg!(debug_assertions) || cfg!(feature = "devtools");
        let failure = {
            let state = self.inner.state.borrow();
            let repeated_at = full_tracking.then(|| {
                node.id.and_then(|id| {
                    state
                        .active
                        .iter()
                        .position(|active| active.phase == node.phase && active.id == Some(id))
                })
            });
            if let Some(position) = repeated_at.flatten() {
                Some((
                    format!(
                        "the same retained node re-entered {} before its previous invocation completed",
                        node.phase
                    ),
                    Some(position),
                ))
            } else if next_depth > state.limit {
                Some((
                    format!(
                        "active frame-pipeline depth {next_depth} exceeded the configured limit {}",
                        state.limit
                    ),
                    None,
                ))
            } else {
                None
            }
        };

        if let Some((reason, repeated_at)) = failure {
            self.fail(node, reason, repeated_at);
        }

        self.inner.depth.set(next_depth);
        if full_tracking {
            self.inner.state.borrow_mut().active.push(node);
        }
        ActivePhaseGuard {
            diagnostics: self.clone(),
            full_tracking,
        }
    }

    fn fail(&self, node: DiagnosticNode, reason: String, repeated_at: Option<usize>) -> ! {
        let mut report = {
            let state = self.inner.state.borrow();
            let path_start = repeated_at.unwrap_or(0).min(state.active.len());
            let mut widget_path = state.active[path_start..]
                .iter()
                .filter(|entry| entry.id.is_some())
                .map(format_node)
                .collect::<Vec<_>>();
            widget_path.push(format_node(&node));
            RecursionReport {
                phase: node.phase,
                reason,
                reentered: repeated_at.is_some(),
                trigger: state.trigger.clone(),
                repeated_node: format_node(&node),
                constraints: node.constraints.map(|value| format!("{value:?}")),
                widget_path,
                last_external_call: state.external_calls.last().cloned(),
                backtrace: Backtrace::force_capture().to_string(),
                log_path: None,
            }
        };
        report.log_path = persist_report(&self.inner.state.borrow().report_directory, &report);
        self.inner.state.borrow_mut().last_report = Some(report.clone());
        panic!("{report}");
    }

    pub(crate) fn external_call(&self, name: impl Into<String>) -> ExternalCallGuard {
        self.inner
            .state
            .borrow_mut()
            .external_calls
            .push(name.into());
        ExternalCallGuard {
            diagnostics: self.clone(),
        }
    }

    pub(crate) fn set_trigger(&self, trigger: impl Into<String>) {
        self.inner.state.borrow_mut().trigger = Some(trigger.into());
    }

    pub(crate) fn last_report(&self) -> Option<RecursionReport> {
        self.inner.state.borrow().last_report.clone()
    }

    pub(crate) fn set_report_directory(&self, path: impl Into<PathBuf>) {
        self.inner.state.borrow_mut().report_directory = path.into();
    }

    pub(crate) fn set_limit(&self, limit: usize) {
        self.inner.state.borrow_mut().limit = limit.clamp(MIN_RECURSION_LIMIT, MAX_RECURSION_LIMIT);
    }
}

pub(crate) struct ActivePhaseGuard {
    diagnostics: RecursionDiagnostics,
    full_tracking: bool,
}

impl Drop for ActivePhaseGuard {
    fn drop(&mut self) {
        if self.full_tracking {
            self.diagnostics.inner.state.borrow_mut().active.pop();
        }
        self.diagnostics
            .inner
            .depth
            .set(self.diagnostics.inner.depth.get().saturating_sub(1));
    }
}

pub(crate) struct ExternalCallGuard {
    diagnostics: RecursionDiagnostics,
}

impl Drop for ExternalCallGuard {
    fn drop(&mut self) {
        self.diagnostics
            .inner
            .state
            .borrow_mut()
            .external_calls
            .pop();
    }
}

fn format_node(node: &DiagnosticNode) -> String {
    match node.id {
        Some(DiagnosticNodeId::Element(id)) => format!("ElementId({:?}) {}", id.0, node.kind),
        Some(DiagnosticNodeId::Render(id)) => format!("RenderObjectId({:?}) {}", id.0, node.kind),
        None => node.kind.to_owned(),
    }
}

fn configured_recursion_limit() -> usize {
    std::env::var("INCULAR_RECURSION_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_RECURSION_LIMIT)
        .clamp(MIN_RECURSION_LIMIT, MAX_RECURSION_LIMIT)
}

fn default_report_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("INCULAR_CRASH_REPORT_DIR") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "windows")]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(path).join("Incular").join("CrashReports");
    }
    #[cfg(target_os = "macos")]
    if let Some(path) = std::env::var_os("HOME") {
        return PathBuf::from(path).join("Library/Logs/Incular");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
            return PathBuf::from(path).join("incular/crashes");
        }
        if let Some(path) = std::env::var_os("HOME") {
            return PathBuf::from(path).join(".local/state/incular/crashes");
        }
    }
    std::env::temp_dir().join("incular-crashes")
}

fn persist_report(directory: &Path, report: &RecursionReport) -> Option<PathBuf> {
    fs::create_dir_all(directory).ok()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = directory.join(format!("recursion-{}-{timestamp}.log", std::process::id()));
    fs::write(&path, report.render(false)).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_render_is_reported_before_stack_exhaustion() {
        let diagnostics = RecursionDiagnostics::new();
        let directory =
            std::env::temp_dir().join(format!("incular-recursion-test-{}", std::process::id()));
        diagnostics.set_report_directory(&directory);
        diagnostics.set_trigger("click(\"Open menu\")");
        let node = DiagnosticNode {
            phase: FramePhase::Layout,
            id: Some(DiagnosticNodeId::Render(RenderObjectId(
                incular_core::ArenaId::from_parts(113, 0),
            ))),
            kind: "Decorated",
            constraints: None,
        };
        let first = diagnostics.enter(node);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _second = diagnostics.enter(node);
        }));
        drop(first);

        assert!(panic.is_err());
        let report = diagnostics.last_report().expect("recursion report");
        assert_eq!(report.phase, FramePhase::Layout);
        assert!(report.reentered);
        assert_eq!(report.trigger.as_deref(), Some("click(\"Open menu\")"));
        assert!(report.repeated_node.contains("Decorated"));
        assert!(report.log_path.as_ref().is_some_and(|path| path.exists()));
        if directory.exists() {
            let _ = fs::remove_dir_all(directory);
        }
    }

    #[test]
    fn excessive_unique_depth_is_reported_as_depth_not_reentry() {
        let diagnostics = RecursionDiagnostics::new();
        diagnostics.set_limit(MIN_RECURSION_LIMIT);
        let directory =
            std::env::temp_dir().join(format!("incular-depth-test-{}", std::process::id()));
        diagnostics.set_report_directory(&directory);

        let mut guards = Vec::new();
        for index in 0..MIN_RECURSION_LIMIT {
            guards.push(diagnostics.enter(DiagnosticNode {
                phase: FramePhase::Paint,
                id: Some(DiagnosticNodeId::Render(RenderObjectId(
                    incular_core::ArenaId::from_parts(index as u32, 0),
                ))),
                kind: "Container",
                constraints: None,
            }));
        }
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _too_deep = diagnostics.enter(DiagnosticNode {
                phase: FramePhase::Paint,
                id: Some(DiagnosticNodeId::Render(RenderObjectId(
                    incular_core::ArenaId::from_parts(MIN_RECURSION_LIMIT as u32, 0),
                ))),
                kind: "Text",
                constraints: None,
            });
        }));
        drop(guards);

        assert!(panic.is_err());
        let report = diagnostics.last_report().expect("depth report");
        assert!(!report.reentered);
        assert!(report.reason.contains("exceeded the configured limit"));
        assert!(!report.to_string().contains("entered twice"));
        if directory.exists() {
            let _ = fs::remove_dir_all(directory);
        }
    }
}
