//! PTY wrapper for container commands.
//!
//! This binary wraps a command in a pseudo-terminal to ensure proper
//! stdout behavior when the parent process uses pipes.
//!
//! It strips ANSI escape sequences and normalizes line endings for clean
//! JSON output when used with stream-json mode.
//!
//! Usage: pty-wrap <command> [args...]

use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

/// Strip ANSI escape sequences and normalize line endings.
/// Converts \r\n to \n and removes standalone \r.
fn strip_ansi_and_normalize(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        let b = data[i];

        // Check for ESC sequence
        if b == 0x1b && i + 1 < data.len() {
            let next = data[i + 1];

            // CSI sequence: ESC [ ... <letter>
            if next == b'[' {
                i += 2;
                // Skip until we hit a letter (the terminator)
                while i < data.len() {
                    let c = data[i];
                    i += 1;
                    if c.is_ascii_alphabetic() || c == b'?' || c == b'~' {
                        // Some sequences end with ?, some with letters
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }

            // OSC sequence: ESC ] ... BEL or ESC \
            if next == b']' {
                i += 2;
                while i < data.len() {
                    if data[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // Simple ESC sequence (e.g., ESC =, ESC >)
            if next.is_ascii_alphabetic() || next == b'=' || next == b'>' || next == b'<' {
                i += 2;
                continue;
            }
        }

        // Handle carriage return
        if b == b'\r' {
            // Skip \r if followed by \n, otherwise treat as \n
            if i + 1 < data.len() && data[i + 1] == b'\n' {
                i += 1; // Skip \r, let next iteration handle \n
                continue;
            }
            // Standalone \r - skip it
            i += 1;
            continue;
        }

        result.push(b);
        i += 1;
    }

    result
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: pty-wrap <command> [args...]");
        std::process::exit(1);
    }

    let debug = std::env::var("PTY_WRAP_DEBUG").is_ok();
    if debug {
        eprintln!("pty-wrap: starting with args: {:?}", args);
    }

    if let Err(e) = run(&args, debug) {
        eprintln!("pty-wrap error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &[String], debug: bool) -> anyhow::Result<()> {
    let pty_system = native_pty_system();

    // Create a PTY with reasonable size
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    if debug {
        eprintln!("pty-wrap: PTY created");
    }

    // Build the command
    let mut cmd = CommandBuilder::new(&args[0]);
    for arg in &args[1..] {
        cmd.arg(arg);
    }

    // Spawn the command in the PTY
    let mut child = pair.slave.spawn_command(cmd)?;

    if debug {
        eprintln!("pty-wrap: child spawned");
    }

    // Get the master PTY for reading/writing
    let master = pair.master;

    // Channel to signal when child exits
    let (done_tx, done_rx) = mpsc::channel::<()>();

    // Spawn thread to read from PTY master, strip ANSI, and write to our stdout
    let mut reader = master.try_clone_reader()?;
    let debug_stdout = debug;
    let stdout_handle = thread::spawn(move || {
        let mut stdout = std::io::stdout();
        let mut buf = [0u8; 4096];
        let mut total_bytes = 0usize;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    if debug_stdout {
                        eprintln!("pty-wrap: stdout reader: EOF, total bytes read: {}", total_bytes);
                    }
                    break;
                }
                Ok(n) => {
                    total_bytes += n;
                    if debug_stdout {
                        eprintln!("pty-wrap: stdout reader: read {} bytes (total: {}): {:?}", n, total_bytes, &buf[..n]);
                    }
                    let cleaned = strip_ansi_and_normalize(&buf[..n]);
                    if !cleaned.is_empty() {
                        if debug_stdout {
                            eprintln!("pty-wrap: stdout reader: writing {} cleaned bytes", cleaned.len());
                        }
                        if stdout.write_all(&cleaned).is_err() {
                            break;
                        }
                        let _ = stdout.flush();
                    }
                }
                Err(e) => {
                    if debug_stdout {
                        eprintln!("pty-wrap: stdout reader: error: {}", e);
                    }
                    break;
                }
            }
        }
    });

    // Spawn thread to read from our stdin and write to PTY master
    let mut writer = master.take_writer()?;
    let debug_stdin = debug;
    let stdin_handle = thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => {
                    if debug_stdin {
                        eprintln!("pty-wrap: stdin reader: EOF");
                    }
                    break;
                }
                Ok(n) => {
                    if debug_stdin {
                        eprintln!("pty-wrap: stdin reader: read {} bytes, forwarding to PTY", n);
                    }
                    if writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = writer.flush();
                }
                Err(e) => {
                    if debug_stdin {
                        eprintln!("pty-wrap: stdin reader: error: {}", e);
                    }
                    break;
                }
            }
        }
        // Signal that stdin is closed
        let _ = done_tx.send(());
    });

    // Wait for the child to exit
    if debug {
        eprintln!("pty-wrap: waiting for child to exit");
    }
    let status = child.wait()?;
    if debug {
        eprintln!("pty-wrap: child exited with code: {}", status.exit_code());
    }

    // Give threads a moment to flush
    let _ = done_rx.recv_timeout(std::time::Duration::from_millis(100));

    // Clean up threads (they'll exit when the PTY closes)
    drop(master);
    let _ = stdout_handle.join();
    let _ = stdin_handle.join();

    // Exit with same code as child
    std::process::exit(status.exit_code() as i32);
}
