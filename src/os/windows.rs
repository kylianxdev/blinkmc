use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::ntdef::NULL;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
use winapi::um::winnt::PROCESS_TERMINATE;

/// Force kill process on Windows.
///
/// This is unsafe because the PID isn't checked.
pub unsafe fn force_kill(pid: u32) -> bool {
    debug!(target: "lazymc", "Sending force kill to {} to kill server", pid);
    let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
    if handle == NULL {
        warn!(target: "lazymc", "Failed to open process handle in order to kill it");
        return false;
    }

    let terminated = TerminateProcess(handle, 1) == TRUE;
    let closed = CloseHandle(handle) == TRUE;

    terminated && closed
}

use winapi::shared::minwindef::DWORD;
use winapi::um::winnt::{HANDLE, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_SUSPEND_RESUME};

#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(process: HANDLE) -> i32;
    fn NtResumeProcess(process: HANDLE) -> i32;
}

/// Pause (or continue) every thread of a process, like SIGSTOP and SIGCONT on Linux.
pub unsafe fn suspend(pid: u32, resume: bool) -> bool {
    let handle = OpenProcess(PROCESS_SUSPEND_RESUME, FALSE, pid as DWORD);
    if handle == NULL {
        warn!(target: "lazymc", "Failed to open the server process to freeze or thaw it");
        return false;
    }
    let status = if resume { NtResumeProcess(handle) } else { NtSuspendProcess(handle) };
    CloseHandle(handle);
    status >= 0
}

/// Empty a process's working set: Windows moves its pages to the compressed memory store and the
/// page file, and gives them back page by page when the process touches them again.
pub unsafe fn trim(pid: u32) -> bool {
    let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, FALSE, pid as DWORD);
    if handle == NULL {
        return false;
    }
    let ok = winapi::um::psapi::EmptyWorkingSet(handle) == TRUE;
    CloseHandle(handle);
    ok
}
