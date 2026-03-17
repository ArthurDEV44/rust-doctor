use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Outcome of a subprocess run with a timeout watchdog.
pub struct ProcessOutput {
    /// Raw stdout content (capped at `max_output_bytes`).
    pub stdout: String,
    /// Whether the process was killed due to timeout.
    pub timed_out: bool,
    /// Exit status code, if the process completed.
    pub exit_code: Option<i32>,
}

/// Spawn a command with a cancellable timeout watchdog.
///
/// Reads stdout up to `max_output_bytes` (stderr is suppressed).
/// If the process exceeds `timeout_secs`, it is killed and `timed_out` is set.
pub fn run_with_timeout(
    mut child: Child,
    timeout_secs: u64,
    max_output_bytes: u64,
) -> Result<ProcessOutput, String> {
    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture subprocess stdout")?;

    // Cancellable timeout watchdog
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let child = Arc::new(Mutex::new(child));
    let child_watcher = Arc::clone(&child);
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_watcher = Arc::clone(&timed_out);

    let watcher = thread::spawn(move || {
        if cancel_rx
            .recv_timeout(Duration::from_secs(timeout_secs))
            .is_err()
            && let Ok(mut c) = child_watcher.lock()
            && matches!(c.try_wait(), Ok(None))
        {
            let _ = c.kill();
            let _ = c.wait(); // Reap to avoid zombie
            timed_out_watcher.store(true, Ordering::Relaxed);
        }
    });

    // Read stdout with a cap to prevent OOM
    let mut output = String::new();
    {
        use std::io::Read;
        let _ = stdout.take(max_output_bytes).read_to_string(&mut output);
    }

    // Cancel watchdog and reap child
    let _ = cancel_tx.send(());
    let _ = watcher.join();

    let exit_code = child
        .lock()
        .ok()
        .and_then(|mut c| c.wait().ok().and_then(|s| s.code()));

    Ok(ProcessOutput {
        stdout: output,
        timed_out: timed_out.load(Ordering::Relaxed),
        exit_code,
    })
}
