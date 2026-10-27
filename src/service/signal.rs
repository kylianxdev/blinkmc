use std::sync::Arc;

use crate::config::Config;
use crate::server::{self, Server};
use crate::util::error;

/// Signal handler task.
pub async fn service(config: Arc<Config>, server: Arc<Server>) {
    loop {
        // Wait for SIGTERM/SIGINT signal
        tokio::signal::ctrl_c().await.unwrap();

        // Quit if there is no server process (a frozen server still has one)
        if server.pid.lock().await.is_none() {
            quit();
        }

        // Stop the server process for real, even when it is frozen, so it saves the world
        if !server.stop_for_real(&config).await {
            quit();
        }

        // Quit once the process is gone, so a service manager can stop lazymc with one signal
        while server.pid.lock().await.is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        quit();
    }
}

/// Gracefully quit.
fn quit() -> ! {
    // TODO: gracefully quit self
    error::quit();
}
