//! Terminal renderers for service-check progress.
//!
//! Interactive terminals get a live spinner block that redraws in place and
//! disappears when the checks finish (cargo/vitest style); dumb terminals get
//! the plain line-per-event output; non-terminals get nothing.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use md_db::progress::{PlainProgress, ProgressSink};

/// Pick the progress renderer for stderr: live spinners on an interactive
/// terminal, plain lines on a dumb terminal, silence when piped.
pub fn stderr_sink() -> Option<Arc<dyn ProgressSink>> {
    if !std::io::stderr().is_terminal() {
        return None;
    }
    let dumb = std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false);
    if dumb {
        Some(Arc::new(PlainProgress))
    } else {
        Some(Arc::new(SpinnerProgress::default()))
    }
}

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const TICK: Duration = Duration::from_millis(100);

struct RunningCheck {
    location: String,
    phase: String,
    command: String,
    started: Instant,
}

#[derive(Default)]
struct State {
    total: usize,
    done: usize,
    failed: usize,
    active: Vec<RunningCheck>,
}

/// Live multi-line spinner block on stderr. One header line with overall
/// progress, one line per running check. Redraws in place ~10×/s and erases
/// itself when [`ProgressSink::end`] is called, leaving the terminal clean
/// for diagnostics and the duration summary.
#[derive(Default)]
pub struct SpinnerProgress {
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ProgressSink for SpinnerProgress {
    fn begin(&self, total_checks: usize, _services: usize, _jobs: usize) {
        if let Ok(mut st) = self.state.lock() {
            st.total = total_checks;
        }
        let state = Arc::clone(&self.state);
        let stop = Arc::clone(&self.stop);
        let handle = std::thread::spawn(move || render_loop(&state, &stop));
        if let Ok(mut slot) = self.handle.lock() {
            *slot = Some(handle);
        }
    }

    fn check_started(&self, location: &str, phase: &str, command: &str) {
        if let Ok(mut st) = self.state.lock() {
            st.active.push(RunningCheck {
                location: location.to_string(),
                phase: phase.to_string(),
                command: command.to_string(),
                started: Instant::now(),
            });
        }
    }

    fn check_finished(&self, location: &str, phase: &str, _tool: &str, ok: bool, _secs: f32) {
        if let Ok(mut st) = self.state.lock() {
            st.active
                .retain(|c| !(c.location == location && c.phase == phase));
            st.done += 1;
            if !ok {
                st.failed += 1;
            }
        }
    }

    fn notice(&self, message: &str) {
        // Notices are emitted during the serial install phase, before begin()
        // starts the render thread — plain lines are safe here.
        eprintln!("dg: {message}");
    }

    fn end(&self) {
        self.stop.store(true, Ordering::Relaxed);
        let handle = self.handle.lock().ok().and_then(|mut h| h.take());
        if let Some(h) = handle {
            let _ = h.join();
        }
    }
}

impl Drop for SpinnerProgress {
    fn drop(&mut self) {
        // Safety net if end() was never reached (e.g. an early return).
        self.end();
    }
}

fn render_loop(state: &Mutex<State>, stop: &AtomicBool) {
    let mut stderr = std::io::stderr().lock();
    let mut drawn = 0usize;
    let mut tick = 0usize;
    while !stop.load(Ordering::Relaxed) {
        if let Ok(st) = state.lock() {
            draw(&mut stderr, &st, FRAMES[tick % FRAMES.len()], &mut drawn);
        }
        tick += 1;
        std::thread::sleep(TICK);
    }
    // Erase the live block; results and the duration summary follow.
    if drawn > 0 {
        let _ = write!(stderr, "\x1b[{drawn}A\r\x1b[J");
        let _ = stderr.flush();
    }
}

fn draw(out: &mut impl Write, st: &State, frame: &str, drawn: &mut usize) {
    let width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .max(20);

    let mut lines = Vec::with_capacity(st.active.len() + 1);
    let failed = if st.failed > 0 {
        format!(", {} failed", st.failed)
    } else {
        String::new()
    };
    lines.push(format!(
        "{frame} dg: {}/{} checks done{failed}, {} running",
        st.done,
        st.total,
        st.active.len()
    ));
    for c in &st.active {
        let secs = c.started.elapsed().as_secs_f32();
        lines.push(format!(
            "  {frame} {} {} · {} · {secs:.1}s",
            c.location, c.phase, c.command
        ));
    }

    // Move to the top of the previous block, then overwrite line by line;
    // \x1b[J clears leftovers when the block shrank. Lines are truncated to
    // the terminal width so they never wrap (wrapping would break the
    // cursor-up arithmetic).
    let mut buf = String::new();
    if *drawn > 0 {
        buf.push_str(&format!("\x1b[{}A", *drawn));
    }
    buf.push('\r');
    for line in &lines {
        buf.push_str("\x1b[2K");
        buf.push_str(&truncate(line, width));
        buf.push('\n');
    }
    buf.push_str("\x1b[J");
    let _ = out.write_all(buf.as_bytes());
    let _ = out.flush();
    *drawn = lines.len();
}

/// Truncate to `width` terminal cells (all characters used here are
/// single-width), appending an ellipsis when the line was cut.
fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_line_unchanged() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("abc", 3), "abc");
    }

    #[test]
    fn truncate_long_line_gets_ellipsis() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abcdef", 4).chars().count(), 4);
    }

    #[test]
    fn spinner_state_tracks_checks() {
        let sp = SpinnerProgress::default();
        // No begin() → no render thread; state updates must still be safe.
        sp.check_started("services/api/", "lint", "mix credo");
        sp.check_started("services/api/", "test", "mix test");
        sp.check_finished("services/api/", "lint", "Credo", true, 1.0);
        sp.check_finished("services/api/", "test", "ExUnit", false, 2.0);
        let st = sp.state.lock().unwrap();
        assert!(st.active.is_empty());
        assert_eq!(st.done, 2);
        assert_eq!(st.failed, 1);
    }
}
