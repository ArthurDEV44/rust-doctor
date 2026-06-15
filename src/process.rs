use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::Duration;

/// Check if a cargo subcommand (e.g. "audit", "deny", "clippy") is installed.
/// Results are cached for the process lifetime.
pub fn is_cargo_subcommand_available(name: &str) -> bool {
    static CACHE: LazyLock<Mutex<HashMap<String, bool>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let cache = CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(&result) = cache.get(name) {
        return result;
    }
    drop(cache);

    let available = Command::new("cargo")
        .args([name, "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(name.to_string(), available);
    available
}

/// Outcome of a subprocess run with a timeout watchdog.
pub struct ProcessOutput {
    /// Raw stdout content (capped at `max_output_bytes`).
    pub stdout: String,
    /// Whether the process was killed due to timeout.
    pub timed_out: bool,
    /// Exit status code, if the process completed.
    pub exit_code: Option<i32>,
}

/// Spawn a command as a new process-group leader so its descendants can be
/// killed as a group later (US-008).
///
/// On Unix, `process_group(0)` (std, stable since 1.64 — no `unsafe`, unlike a
/// hand-rolled `setpgid` via `pre_exec`, which `#![forbid(unsafe_code)]` would
/// reject) makes the child's PGID equal its PID. On other platforms this is a
/// plain spawn and the watchdog falls back to killing the direct child.
pub fn spawn_in_group(cmd: &mut Command) -> std::io::Result<Child> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// Kill a child process and (on Unix) its whole process group, so the `rustc`
/// children of a `cargo` subprocess are terminated too — `Child::kill()` alone
/// only reaps the direct child and leaves orphans (US-008).
///
/// The reparented grandchildren are reaped by init after the group SIGKILL; the
/// direct child is reaped by the caller's `wait()`. No `unsafe`: the process-group
/// SIGKILL goes through rustix's safe API.
pub fn kill_process_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // pgid == child.id() because `spawn_in_group` made the child a group leader.
        if let Ok(raw) = i32::try_from(child.id())
            && let Some(pid) = rustix::process::Pid::from_raw(raw)
        {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
            return;
        }
    }
    let _ = child.kill();
}

/// Spawn a command with a cancellable timeout watchdog.
///
/// Reads stdout up to `max_output_bytes` (stderr is suppressed).
/// If the process exceeds `timeout_secs`, its whole process group is killed
/// (US-008) and `timed_out` is set.
pub fn run_with_timeout(
    mut child: Child,
    timeout_secs: u64,
    max_output_bytes: u64,
) -> Result<ProcessOutput, String> {
    tracing::debug!(
        timeout_secs,
        max_output_bytes,
        "running subprocess with timeout"
    );
    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture subprocess stdout")?;

    // Drain stderr in background to prevent pipe deadlock if caller piped it
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            use std::io::Read;
            let _ = std::io::copy(&mut stderr.take(1024 * 1024), &mut std::io::sink());
        });
    }

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
            kill_process_tree(&mut c); // SIGKILL the whole group, not just `cargo`
            let _ = c.wait(); // Reap the direct child to avoid a zombie
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

    // Reap (or re-reap) the child. If the watchdog already killed+waited it on
    // timeout, this second `wait()` is a benign no-op (`ECHILD`, swallowed by
    // `.ok()`) — it is intentionally NOT removed, since dropping it would leak a
    // zombie on the happy path where the watchdog never fired.
    let exit_code = child
        .lock()
        .ok()
        .and_then(|mut c| c.wait().ok().and_then(|s| s.code()));

    let did_timeout = timed_out.load(Ordering::Relaxed);
    tracing::debug!(exit_code, timed_out = did_timeout, "subprocess finished");

    Ok(ProcessOutput {
        stdout: output,
        timed_out: did_timeout,
        exit_code,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::{kill_process_tree, spawn_in_group};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    // --- US-008: process-group spawn + tree kill ---

    #[test]
    fn test_spawn_in_group_leads_own_group() {
        // `sh -c 'sleep 30'`: sh is the direct child, `sleep` a grandchild — both
        // share the new group created by `process_group(0)`.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_in_group(&mut cmd).expect("spawn should succeed");

        let raw = i32::try_from(child.id()).unwrap();
        let child_pid = rustix::process::Pid::from_raw(raw).unwrap();
        // process_group(0) makes the child a group leader → its PGID equals its PID.
        let group = rustix::process::getpgid(Some(child_pid)).expect("getpgid");
        assert_eq!(group, child_pid, "child should lead its own process group");

        kill_process_tree(&mut child);
        let _ = child.wait();
    }

    #[test]
    fn test_kill_process_tree_terminates_child_promptly() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_in_group(&mut cmd).expect("spawn should succeed");

        // Sanity: it is still running before the kill.
        assert!(
            matches!(child.try_wait(), Ok(None)),
            "child should be alive"
        );

        kill_process_tree(&mut child);
        let start = Instant::now();
        let status = child.wait().expect("wait should reap the killed child");
        assert!(
            !status.success(),
            "a SIGKILL'd process must not report success"
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "group kill should terminate the child promptly"
        );
    }
}
