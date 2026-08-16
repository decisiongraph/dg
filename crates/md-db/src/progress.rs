//! Progress reporting for long-running operations (service checks).
//!
//! The library only emits events through [`ProgressSink`]; binaries decide
//! how to render them (plain lines, interactive spinners, or silence).

/// Receives service-check progress events. Implementations must be
/// thread-safe: checks run in parallel and report from worker threads.
pub trait ProgressSink: Send + Sync {
    /// Checks are about to run.
    fn begin(&self, total_checks: usize, services: usize, jobs: usize);
    /// One check command started.
    fn check_started(&self, location: &str, phase: &str, command: &str);
    /// One check command finished.
    fn check_finished(&self, location: &str, phase: &str, tool: &str, ok: bool, secs: f32);
    /// One-off informational message (e.g. installing JS dependencies).
    fn notice(&self, message: &str);
    /// All checks finished; release any terminal state.
    fn end(&self);
}

/// Line-per-event renderer for non-interactive terminals.
pub struct PlainProgress;

impl ProgressSink for PlainProgress {
    fn begin(&self, total_checks: usize, services: usize, jobs: usize) {
        eprintln!(
            "dg: running {total_checks} check(s) across {services} service(s), {jobs} service(s) in parallel"
        );
    }

    fn check_started(&self, location: &str, phase: &str, command: &str) {
        eprintln!("dg: → {location} {phase}: {command}");
    }

    fn check_finished(&self, location: &str, phase: &str, tool: &str, ok: bool, secs: f32) {
        let mark = if ok { "✓" } else { "✗" };
        eprintln!("dg: {mark} {location} {phase} ({tool}) {secs:.1}s");
    }

    fn notice(&self, message: &str) {
        eprintln!("dg: {message}");
    }

    fn end(&self) {}
}
