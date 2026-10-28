#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};

// The server runs in its own process group (Unix) or is the root of its own process tree
// (Windows). Signals go to all of it, so a `command` that is a script (start.sh, run.sh) freezes,
// thaws and stops together with the Java process it started.

/// Force kill process.
///
/// Results in undefined behavior if PID is invalid.
#[allow(unreachable_code)]
pub fn force_kill(pid: u32) -> bool {
    #[cfg(unix)]
    return unix_signal(pid, Signal::SIGKILL);

    #[cfg(windows)]
    unsafe {
        return windows::force_kill(pid);
    }

    unimplemented!("force killing Minecraft server process not implemented on this platform");
}

/// Gracefully kill process.
/// Results in undefined behavior if PID is invalid.
///
/// # Panics
/// Panics on platforms other than Unix.
#[allow(unreachable_code, dead_code, unused_variables)]
pub fn kill_gracefully(pid: u32) -> bool {
    #[cfg(unix)]
    return unix_signal(pid, Signal::SIGTERM);

    unimplemented!(
        "gracefully killing Minecraft server process not implemented on non-Unix platforms"
    );
}

/// Freeze (pause) a process: SIGSTOP on Linux and macOS, NtSuspendProcess on Windows.
#[allow(unreachable_code)]
pub fn freeze(pid: u32) -> bool {
    #[cfg(unix)]
    return unix_signal(pid, Signal::SIGSTOP);

    #[cfg(windows)]
    return unsafe { windows::suspend(pid, false) };

    false
}

/// Unfreeze (continue) a process: SIGCONT on Linux and macOS, NtResumeProcess on Windows.
#[allow(unreachable_code)]
pub fn unfreeze(pid: u32) -> bool {
    #[cfg(unix)]
    return unix_signal(pid, Signal::SIGCONT);

    #[cfg(windows)]
    return unsafe { windows::suspend(pid, true) };

    false
}

/// Send a signal to the server's process group (its PID is the group ID), or to the process alone
/// if it has no group of its own.
#[cfg(unix)]
pub fn unix_signal(pid: u32, signal: Signal) -> bool {
    let group = signal::killpg(Pid::from_raw(pid as i32), signal);
    match group.or_else(|_| signal::kill(Pid::from_raw(pid as i32), signal)) {
        Ok(()) => true,
        Err(err) => {
            warn!(target: "lazymc", "Sending {signal} signal to server failed: {err}");
            false
        }
    }
}
