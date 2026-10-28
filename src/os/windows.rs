use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::ntdef::NULL;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use winapi::um::winnt::PROCESS_TERMINATE;

/// Force kill process on Windows, and everything it started.
///
/// This is unsafe because the PID isn't checked.
pub unsafe fn force_kill(pid: u32) -> bool {
    debug!(target: "lazymc", "Sending force kill to {} to kill server", pid);
    let mut killed = true;
    for pid in tree(pid).into_iter().rev() {
        let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
        if handle == NULL {
            warn!(target: "lazymc", "Failed to open process handle in order to kill it");
            killed = false;
            continue;
        }
        killed &= TerminateProcess(handle, 1) == TRUE;
        CloseHandle(handle);
    }
    killed
}

use winapi::shared::minwindef::DWORD;
use winapi::um::winnt::{HANDLE, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_SUSPEND_RESUME};

#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(process: HANDLE) -> i32;
    fn NtResumeProcess(process: HANDLE) -> i32;
}

/// Pause (or continue) every thread of a process and of everything it started, like SIGSTOP and
/// SIGCONT on a process group on Linux. A `command` that is a .bat file freezes with its Java.
pub unsafe fn suspend(pid: u32, resume: bool) -> bool {
    let mut ok = true;
    for pid in tree(pid) {
        let handle = OpenProcess(PROCESS_SUSPEND_RESUME, FALSE, pid as DWORD);
        if handle == NULL {
            warn!(target: "lazymc", "Failed to open the server process to freeze or thaw it");
            ok = false;
            continue;
        }
        let status = if resume { NtResumeProcess(handle) } else { NtSuspendProcess(handle) };
        CloseHandle(handle);
        ok &= status >= 0;
    }
    ok
}

/// Empty the working set of a process and everything it started: Windows moves those pages to the
/// compressed memory store and the page file, and gives them back page by page when the process
/// touches them again.
pub unsafe fn trim(pid: u32) -> bool {
    let mut ok = false;
    for pid in tree(pid) {
        let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, FALSE, pid as DWORD);
        if handle == NULL {
            continue;
        }
        ok |= winapi::um::psapi::EmptyWorkingSet(handle) == TRUE;
        CloseHandle(handle);
    }
    ok
}

/// The process and every process below it, parents before their children.
fn tree(pid: u32) -> Vec<u32> {
    let mut pairs = vec![];
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot != INVALID_HANDLE_VALUE {
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as DWORD;
            let mut more = Process32FirstW(snapshot, &mut entry) == TRUE;
            while more {
                pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
                more = Process32NextW(snapshot, &mut entry) == TRUE;
            }
            CloseHandle(snapshot);
        }
    }
    let mut out = vec![pid];
    let mut i = 0;
    while i < out.len() {
        let parent = out[i];
        for &(child, of) in &pairs {
            if of == parent && child != parent && child != 0 && !out.contains(&child) {
                out.push(child);
            }
        }
        i += 1;
    }
    out
}
