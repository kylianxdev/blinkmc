//! Blink additions: the server console, saving before a freeze, and moving a frozen server's memory
//! out of RAM.
//!
//! The server runs in its own process group with its stdin and stdout piped through Blink. That
//! way Blink can type `save-all` itself and see "Saved the game", while you still use the console
//! as before (what you type goes to the server, what it prints shows up here). Ctrl+C reaches only
//! Blink, which stops the server properly, also when it is frozen.

#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, Notify};

/// Paper's answer to `save-all` once the world is on disk.
const SAVED: &str = "Saved the game";

/// How long a save may take before we freeze anyway.
const SAVE_TIMEOUT: Duration = Duration::from_secs(30);

/// The server console: what we write to it and what it says.
#[derive(Debug, Default)]
pub struct Console {
    stdin: Mutex<Option<ChildStdin>>,
    saves: AtomicU64,
    saved: Notify,
}

impl Console {
    /// Attach to a newly started server process.
    pub fn attach(self: &std::sync::Arc<Self>, stdin: Option<ChildStdin>, stdout: Option<ChildStdout>) {
        let console = self.clone();
        tokio::spawn(async move {
            *console.stdin.lock().await = stdin;
        });

        // Server output: show it, and notice finished saves
        if let Some(stdout) = stdout {
            let console = self.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                let mut out = tokio::io::stdout();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.contains(SAVED) {
                        console.saves.fetch_add(1, Ordering::SeqCst);
                        console.saved.notify_waiters();
                    }
                    let _ = out.write_all(line.as_bytes()).await;
                    let _ = out.write_all(b"\n").await;
                    let _ = out.flush().await;
                }
            });
        }
    }

    /// Forward what is typed in our console to the server, for as long as Blink runs.
    pub fn forward_input(self: &std::sync::Arc<Self>) {
        let console = self.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !console.command(&line).await {
                    eprintln!(" INFO  blink > The server is not running, join to wake it up");
                }
            }
        });
    }

    /// Type a command in the server console. False if no server is running.
    pub async fn command(&self, command: &str) -> bool {
        let mut stdin = self.stdin.lock().await;
        let Some(pipe) = stdin.as_mut() else {
            return false;
        };
        let ok = pipe.write_all(format!("{command}\n").as_bytes()).await.is_ok() && pipe.flush().await.is_ok();
        if !ok {
            *stdin = None;
        }
        ok
    }

    /// Forget the server process (it quit).
    pub async fn detach(&self) {
        *self.stdin.lock().await = None;
    }

    /// Make the server save its world and wait until it is on disk (at most 30 seconds).
    pub async fn save(&self) -> bool {
        let before = self.saves.load(Ordering::SeqCst);
        let saved = self.saved.notified();
        tokio::pin!(saved);
        saved.as_mut().enable();
        if !self.command("save-all").await {
            return false;
        }
        let wait = async {
            while self.saves.load(Ordering::SeqCst) == before {
                saved.as_mut().await;
                saved.set(self.saved.notified());
                saved.as_mut().enable();
            }
        };
        tokio::time::timeout(SAVE_TIMEOUT, wait).await.is_ok()
    }
}

/// The file Blink downloads once a day to be counted (see `usage_stats`).
const USAGE_URL: &str = "https://github.com/kylianxdev/blinkmc/releases/download/usage/active";

/// Anonymous usage count: once a day, download a 1 byte file from Blink's GitHub releases. GitHub
/// counts downloads, so the maintainers see how many servers run Blink. Nothing is sent, and it
/// is skipped without curl. Off with `usage_stats = false` or DO_NOT_TRACK=1.
pub fn usage_stats(enabled: bool) {
    let opted_out = ["DO_NOT_TRACK", "BLINK_NO_USAGE_STATS"]
        .iter()
        .any(|v| std::env::var(v).map(|v| !v.is_empty() && v != "0").unwrap_or(false));
    if !enabled || opted_out {
        return;
    }
    info!(target: "lazymc", "Anonymous usage count is on: one download a day from GitHub, nothing is sent (usage_stats = false turns it off)");
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(10 * 60)).await;
        loop {
            let _ = tokio::process::Command::new("curl")
                .args(["-fsL", "-o", "/dev/null", "--max-time", "30", USAGE_URL])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
            tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
        }
    });
}

#[cfg(target_os = "linux")]
/// Our own cgroup (systemd puts every service in one), if we may ask the kernel to reclaim in it.
fn cgroup() -> Option<PathBuf> {
    let line = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = line.lines().find_map(|l| l.strip_prefix("0::"))?.trim();
    let dir = PathBuf::from("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    dir.join("memory.reclaim").exists().then_some(dir)
}

#[cfg(target_os = "linux")]
/// Is the process stopped (frozen)?
fn is_frozen(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rsplit(')').next().map(|r| r.trim_start().starts_with('T')))
        .unwrap_or(false)
}

/// A process of more than 50 MB in the cgroup that is neither Blink nor the server.
#[cfg(target_os = "linux")]
fn big_neighbour(dir: &std::path::Path, server: u32) -> Option<String> {
    let me = std::process::id();
    let procs = std::fs::read_to_string(dir.join("cgroup.procs")).ok()?;
    procs.lines().filter_map(|l| l.trim().parse::<u32>().ok()).find_map(|pid| {
        if pid == me || pid == server {
            return None;
        }
        let pages: u64 = std::fs::read_to_string(format!("/proc/{pid}/statm"))
            .ok()?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()?;
        let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
        (pages * 4096 > 50 << 20).then(|| name.trim().to_string())
    })
}

#[cfg(target_os = "linux")]
fn read_mb(dir: &std::path::Path, file: &str) -> u64 {
    std::fs::read_to_string(dir.join(file))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
        >> 20
}

/// A few seconds after a freeze, ask the kernel to move the frozen server's memory out of RAM:
/// compressed into zswap first, the coldest part to swap. It comes back by itself, page by page,
/// when the server is thawed.
pub fn reclaim_after_freeze(pid: u32) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        reclaim(pid).await;
    });
}

/// Windows: empty the frozen server's working set; Windows compresses it (memory compression) and
/// pages it out, and the server gets it back when it is thawed.
#[cfg(windows)]
async fn reclaim(pid: u32) {
    if unsafe { crate::os::windows::trim(pid) } {
        info!(target: "lazymc", "Frozen server's memory handed to Windows' memory compression");
    }
}

/// macOS compresses the memory of a paused process by itself when it needs RAM.
#[cfg(not(any(target_os = "linux", windows)))]
async fn reclaim(_pid: u32) {}

/// Linux: ask the kernel to reclaim in our cgroup: zswap first, the coldest part to swap.
#[cfg(target_os = "linux")]
async fn reclaim(pid: u32) {
    if !is_frozen(pid) {
        return;
    }
    let Some(dir) = cgroup() else {
        debug!(target: "lazymc", "No cgroup with memory.reclaim, the kernel will move the frozen server's memory when it needs RAM");
        return;
    };
    // Reclaim works on the whole cgroup. If something big besides Blink and the server shares it (a
    // browser, when Blink runs in a desktop terminal), leave it alone: we'd push that out of RAM too.
    if let Some(other) = big_neighbour(&dir, pid) {
        info!(target: "lazymc", "Not moving the frozen server's memory: {other} shares Blink's cgroup. Run Blink as a service (install.sh) or in tmux for that.");
        return;
    }
    let before = read_mb(&dir, "memory.current");
    // all but 64 MB; swappiness=200: the Java heap first, file cache last
    let request = format!("{}M swappiness=200", before.saturating_sub(64));
    match tokio::fs::write(dir.join("memory.reclaim"), request).await {
        Ok(()) => {}
        // the kernel could not reclaim all of it, which is fine
        Err(err) if err.raw_os_error() == Some(libc::EAGAIN) => {}
        Err(err) => {
            warn!(target: "lazymc", "Could not move the frozen server's memory out of RAM: {err} (see the README, memory.reclaim)");
            return;
        }
    }
    info!(
        target: "lazymc",
        "Frozen server's memory: {} MB -> {} MB in RAM ({} MB compressed in zswap)",
        before,
        read_mb(&dir, "memory.current"),
        read_mb(&dir, "memory.zswap.current"),
    );
}
