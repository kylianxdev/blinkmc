//! Blink additions: the server console, saving before a freeze, and moving a frozen server's memory
//! out of RAM.
//!
//! The server runs in its own process group with its stdin and stdout piped through Blink. That
//! way Blink can type `save-all` itself and see "Saved the game", while you still use the console
//! as before (what you type goes to the server, what it prints shows up here). Ctrl+C reaches only
//! Blink, which stops the server properly, also when it is frozen.

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

/// Our own cgroup (systemd puts every service in one), if we may ask the kernel to reclaim in it.
fn cgroup() -> Option<PathBuf> {
    let line = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = line.lines().find_map(|l| l.strip_prefix("0::"))?.trim();
    let dir = PathBuf::from("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    dir.join("memory.reclaim").exists().then_some(dir)
}

/// Is the process stopped (frozen)?
fn is_frozen(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rsplit(')').next().map(|r| r.trim_start().starts_with('T')))
        .unwrap_or(false)
}

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
        if !is_frozen(pid) {
            return;
        }
        let Some(dir) = cgroup() else {
            debug!(target: "lazymc", "No cgroup with memory.reclaim, the kernel will move the frozen server's memory when it needs RAM");
            return;
        };
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
    });
}
