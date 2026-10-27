use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::config::{Config, Server as ConfigServer};
use crate::mc::ban::{self, BannedIps};
use crate::mc::{server_properties, whitelist};
use crate::server::Server;

/// How often to look for changed server files. Polling a few files costs nothing and needs no
/// extra watcher thread or library.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Service to watch server file changes.
pub fn service(config: Arc<Config>, server: Arc<Server>) {
    // Ensure server directory is set, it must exist
    let dir = match ConfigServer::server_directory(&config) {
        Some(dir) if dir.is_dir() => dir,
        _ => {
            warn!(target: "lazymc", "Server directory doesn't exist, can't watch file changes to reload whitelist and banned IPs");
            return;
        }
    };

    // Keep watching
    #[allow(clippy::blocks_in_conditions)]
    while {
        // Update all files once
        reload_bans(&config, &server, &dir.join(ban::FILE));
        reload_whitelist(&config, &server, &dir);
        reload_known_players(&config, &server, &dir);

        // Watch for changes, update accordingly
        watch_server(&config, &server, &dir)
    } {}
}

/// Watch server directory.
///
/// Returns `true` if we should watch again.
#[must_use]
fn watch_server(config: &Config, server: &Server, dir: &Path) -> bool {
    // Directory must exist
    if !dir.is_dir() {
        error!(target: "lazymc", "Server directory does not exist at {} anymore, not watching changes", dir.display());
        return false;
    }

    // Look at the modification times of the files we care about
    let files = [
        ban::FILE,
        whitelist::WHITELIST_FILE,
        whitelist::OPS_FILE,
        server_properties::FILE,
        USERCACHE_FILE,
    ];
    let modified = |name: &str| std::fs::metadata(dir.join(name)).and_then(|m| m.modified()).ok();
    let mut last: Vec<_> = files.iter().map(|name| modified(name)).collect();

    loop {
        std::thread::sleep(POLL_INTERVAL);
        if !dir.is_dir() {
            error!(target: "lazymc", "Server directory does not exist at {} anymore, not watching changes", dir.display());
            return false;
        }
        for (i, name) in files.iter().enumerate() {
            let now = modified(name);
            if now != last[i] {
                last[i] = now;
                update(config, server, dir, &dir.join(name));
            }
        }
    }
}

/// Process a file change on the given path.
///
/// Should be called both when created, changed or removed.
fn update(config: &Config, server: &Server, dir: &Path, path: &Path) {
    // Update bans
    if path.ends_with(ban::FILE) {
        reload_bans(config, server, path);
    }

    // Update known players
    if path.ends_with(USERCACHE_FILE) {
        reload_known_players(config, server, dir);
    }

    // Update whitelist
    if path.ends_with(whitelist::WHITELIST_FILE)
        || path.ends_with(whitelist::OPS_FILE)
        || path.ends_with(server_properties::FILE)
    {
        reload_whitelist(config, server, dir);
    }
}

/// Players who joined before.
const USERCACHE_FILE: &str = "usercache.json";

/// Reload the names of players who joined before.
fn reload_known_players(config: &Config, server: &Server, dir: &Path) {
    if !config.server.wake_known_players {
        return;
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        name: String,
    }
    let known = std::fs::read_to_string(dir.join(USERCACHE_FILE))
        .ok()
        .and_then(|data| serde_json::from_str::<Vec<Entry>>(&data).ok())
        .map(|entries| entries.into_iter().map(|e| e.name.to_lowercase()).collect::<Vec<_>>());
    debug!(target: "lazymc", "Loaded {} known players", known.as_ref().map_or(0, |k| k.len()));
    futures::executor::block_on(async { *server.known_players.write().await = known });
}

/// Reload banned IPs.
fn reload_bans(config: &Config, server: &Server, path: &Path) {
    // Bans must be enabled
    if !config.server.block_banned_ips && !config.server.drop_banned_ips {
        return;
    }

    trace!(target: "lazymc", "Reloading banned IPs...");

    // File must exist, clear file otherwise
    if !path.is_file() {
        debug!(target: "lazymc", "No banned IPs, {} does not exist", ban::FILE);
        // warn!(target: "lazymc", "Not blocking banned IPs, {} file does not exist", ban::FILE);
        server.set_banned_ips_blocking(BannedIps::default());
        return;
    }

    // Load and update banned IPs
    match ban::load(path) {
        Ok(ips) => server.set_banned_ips_blocking(ips),
        Err(err) => {
            debug!(target: "lazymc", "Failed load banned IPs from {}, ignoring: {}", ban::FILE, err);
        }
    }

    // Show warning if 127.0.0.1 is banned
    if server.is_banned_ip_blocking(&("127.0.0.1".parse().unwrap())) {
        warn!(target: "lazymc", "Local address 127.0.0.1 IP banned, probably not what you want");
        warn!(target: "lazymc", "Use '/pardon-ip 127.0.0.1' on the server to unban");
    }
}

/// Reload whitelisted users.
fn reload_whitelist(config: &Config, server: &Server, dir: &Path) {
    // Whitelist must be enabled
    if !config.server.wake_whitelist {
        return;
    }

    // Must be enabled in server.properties
    let enabled = server_properties::read_property(dir.join(server_properties::FILE), "white-list")
        .map(|v| v.trim() == "true")
        .unwrap_or(false);
    if !enabled {
        server.set_whitelist_blocking(None);
        debug!(target: "lazymc", "Not using whitelist, not enabled in {}", server_properties::FILE);
        return;
    }

    trace!(target: "lazymc", "Reloading whitelisted users...");

    // Load and update whitelisted users
    match whitelist::load_dir(dir) {
        Ok(whitelist) => server.set_whitelist_blocking(Some(whitelist)),
        Err(err) => {
            debug!(target: "lazymc", "Failed load whitelist from {}, ignoring: {}", dir.display(), err);
        }
    }
}
