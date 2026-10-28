//! `blink restore`: undo Blink's changes to the server's own files, so it runs without Blink again.
//! Only the values Blink sets are put back; everything else you changed since stays.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use clap::ArgMatches;

use crate::config::{Config, Server as ConfigServer};
use crate::mc::server_properties;
use crate::util::error::{quit_error, ErrorHintsBuilder};

pub fn invoke(matches: &ArgMatches) {
    let mut path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    if let Some(p) = crate::util::canonicalize(&path) {
        path = p;
    }
    let config = match Config::load(path) {
        Ok(config) => config,
        Err(err) => quit_error(
            anyhow!(err).context("Could not read blink.toml, which says what to restore"),
            ErrorHintsBuilder::default().config(true).verbose(false).build().unwrap(),
        ),
    };
    let dir = ConfigServer::server_directory(&config).unwrap_or_else(|| ".".into());

    // server.properties: the values from before Blink (kept by blink init), else the defaults
    let props = dir.join(server_properties::FILE);
    let before = dir.join("server.properties.before-blink");
    if props.is_file() {
        let port = config.public.address.port().to_string();
        let old = |key: &str, default: &str| {
            fs::read_to_string(&before)
                .ok()
                .and_then(|text| text.lines().find_map(|l| l.strip_prefix(&format!("{key}=")).map(str::to_string)))
                .unwrap_or_else(|| default.to_string())
        };
        let changes = HashMap::from([
            ("server-ip", old("server-ip", "")),
            ("server-port", old("server-port", &port)),
            ("query.port", old("query.port", &port)),
            ("max-tick-time", old("max-tick-time", "60000")),
        ]);
        server_properties::rewrite_file(&props, changes);
        let _ = fs::remove_file(&before);
        eprintln!("server.properties: the server is on port {} again", old("server-port", &port));
    }

    crate::launcher::restore_server_files(&dir, config.server.send_proxy_v2);
    eprintln!("spigot.yml, paper-global.yml and spark: Paper's watchdog and defaults are back");
    eprintln!();
    eprintln!("The server runs without Blink again (start it as you did before). You can delete blink and blink.toml.");
}
