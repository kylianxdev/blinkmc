use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::ArgMatches;
use serde::Deserialize;
use version_compare::Cmp;

use crate::proto;
use crate::util::error::{quit_error, quit_error_msg, ErrorHintsBuilder};
use crate::util::serde::to_socket_addrs;

/// Default configuration file location.
pub const CONFIG_FILE: &str = "blink.toml";

/// Configuration version user should be using, or warning will be shown.
const CONFIG_VERSION: &str = "0.2.8";

/// Load config from file, based on CLI arguments.
///
/// Quits with an error message on failure.
pub fn load(matches: &ArgMatches) -> Config {
    // Get config path, attempt to canonicalize
    let mut path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    if let Some(p) = crate::util::canonicalize(&path) {
        path = p;
    }

    // Ensure configuration file exists
    if !path.is_file() {
        quit_error_msg(
            format!(
                "There is no {} here ({})",
                CONFIG_FILE,
                path.to_str().unwrap_or("?")
            ),
            ErrorHintsBuilder::default()
                .config(true)
                .config_generate(true)
                .verbose(false)
                .build()
                .unwrap(),
        );
    }

    // Load config
    let config = match Config::load(path) {
        Ok(config) => config,
        Err(err) => {
            quit_error(
                anyhow!(err).context("Failed to load config"),
                ErrorHintsBuilder::default()
                    .config(true)
                    .config_test(true)
                    .verbose(false)
                    .build()
                    .unwrap(),
            );
        }
    };

    // Problems that would only show up later (at the first join), now
    if let Err(problems) = config.check() {
        crate::util::error::quit_problems(problems);
    }

    config
}

/// Configuration.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Configuration path if known.
    ///
    /// Should be used as base directory for filesystem operations.
    #[serde(skip)]
    pub path: Option<PathBuf>,

    /// Public configuration.
    #[serde(default)]
    pub public: Public,

    /// Server configuration.
    pub server: Server,

    /// Time configuration.
    #[serde(default)]
    pub time: Time,

    /// MOTD configuration.
    #[serde(default)]
    pub motd: Motd,

    /// Join configuration.
    #[serde(default)]
    pub join: Join,

    /// Lockout feature.
    #[serde(default)]
    pub lockout: Lockout,

    /// RCON configuration.
    #[serde(default)]
    pub rcon: Rcon,

    /// Advanced configuration.
    #[serde(default)]
    pub advanced: Advanced,

    /// Config configuration.
    #[serde(default)]
    pub config: ConfigConfig,
}

impl Config {
    /// Load configuration from file.
    pub fn load(path: PathBuf) -> Result<Self, io::Error> {
        let data = fs::read_to_string(&path)?;
        let mut config: Config = toml::from_str(&data).map_err(io::Error::other)?;

        // Show warning if config version is problematic
        match &config.config.version {
            None => warn!(target: "lazymc::config", "blink.toml has no [config] version, it may be outdated"),
            Some(version) => match version_compare::compare_to(version, CONFIG_VERSION, Cmp::Ge) {
                Ok(false) => {
                    warn!(target: "lazymc::config", "blink.toml is from an older version; 'blink init --force' writes a new one")
                }
                Err(_) => {
                    warn!(target: "lazymc::config", "The [config] version in blink.toml is invalid")
                }
                Ok(true) => {}
            },
        }
        config.path.replace(path);

        Ok(config)
    }

    /// Problems that would stop the server from starting, worst first.
    pub fn check(&self) -> Result<(), Vec<String>> {
        let mut problems = vec![];
        let dir = Server::server_directory(self);
        match shlex::split(&self.server.command).filter(|args| !args.is_empty()) {
            None => problems.push("server.command in blink.toml is empty or has an unclosed quote".to_string()),
            Some(args) if args[0] == "blink-paper" => {
                let settings = crate::launcher::settings(&args[1..]);
                let jar = settings.get("JAR").map_or("paper.jar", String::as_str);
                if let Some(dir) = dir.as_ref().filter(|dir| dir.is_dir()) {
                    if !dir.join(jar).is_file() {
                        problems.push(format!("{jar} is not in {}; set JAR= in server.command in blink.toml", dir.display()));
                    }
                }
                if let Some(java) = settings.get("JAVA") {
                    let java = std::path::Path::new(java);
                    if java.is_absolute() && !java.is_file() {
                        let java = java.display();
                        problems.push(format!("Java is not at {java}; fix JAVA= in server.command in blink.toml, or remove it to use the newest Java"));
                    }
                }
            }
            Some(_) => {}
        }
        if let Some(dir) = dir.as_ref().filter(|dir| !dir.is_dir()) {
            problems.push(format!("The server folder {} does not exist (server.directory in blink.toml)", dir.display()));
        }
        if self.public.address.port() == self.server.address.port()
            && (self.server.address.ip().is_loopback() || self.server.address.ip() == self.public.address.ip())
        {
            problems.push("[public] and [server] use the same port; the server needs a port of its own behind Blink".to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}

/// Public configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Public {
    /// Public address.
    #[serde(deserialize_with = "to_socket_addrs")]
    pub address: SocketAddr,

    /// Minecraft protocol version name hint.
    pub version: String,

    /// Minecraft protocol version hint.
    pub protocol: u32,
}

impl Default for Public {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:25565".parse().unwrap(),
            version: proto::PROTO_DEFAULT_VERSION.to_string(),
            protocol: proto::PROTO_DEFAULT_PROTOCOL,
        }
    }
}

/// Server configuration.
#[derive(Debug, Deserialize)]
pub struct Server {
    /// Server directory.
    ///
    /// Private because you should use `Server::server_directory()` instead.
    #[serde(default = "option_pathbuf_dot")]
    directory: Option<PathBuf>,

    /// Start command.
    pub command: String,

    /// Server address.
    #[serde(
        deserialize_with = "to_socket_addrs",
        default = "server_address_default"
    )]
    pub address: SocketAddr,

    /// Freeze the server process instead of restarting it when no players online, making it start up faster.
    /// Linux, macOS and Windows.
    #[serde(default = "bool_true")]
    pub freeze_process: bool,

    /// Command to run (and wait for, at most 60 seconds) right before the server is frozen, in the
    /// server directory. For example to make the server save its world. If a player joins while it
    /// runs, the server is not frozen.
    #[serde(default)]
    pub before_freeze: Option<String>,

    /// Make the server save its world (`save-all`) and wait for it before it is frozen.
    #[serde(default = "bool_true")]
    pub save_before_freeze: bool,

    /// Immediately wake server when starting Blink.
    #[serde(default)]
    pub wake_on_start: bool,

    /// Immediately wake server after crash.
    #[serde(default)]
    pub wake_on_crash: bool,

    /// Probe required server details when starting Blink, wakes server on start.
    #[serde(default)]
    pub probe_on_start: bool,

    /// Whether this server runs forge.
    #[serde(default)]
    pub forge: bool,

    /// Server starting timeout. Force kill server process if it takes longer.
    #[serde(default = "u32_300")]
    pub start_timeout: u32,

    /// Server stopping timeout. Force kill server process if it takes longer.
    #[serde(default = "u32_150")]
    pub stop_timeout: u32,

    /// To wake server, user must be in server whitelist if enabled on server.
    #[serde(default = "bool_true")]
    pub wake_whitelist: bool,

    /// Without a whitelist: only players who joined before (usercache.json) can wake the server, so
    /// the bots that scan the internet all day can't (new players can't either). Off by default: a
    /// bot that wakes the server only keeps it awake until the next freeze.
    #[serde(default)]
    pub wake_known_players: bool,

    /// Block banned IPs as listed in banned-ips.json in server directory.
    #[serde(default = "bool_true")]
    pub block_banned_ips: bool,

    /// Drop connections from banned IPs.
    #[serde(default)]
    pub drop_banned_ips: bool,

    /// Add HAProxy v2 header to proxied connections.
    #[serde(default)]
    pub send_proxy_v2: bool,
}

impl Server {
    /// Get the server directory.
    ///
    /// This does not check whether it exists.
    pub fn server_directory(config: &Config) -> Option<PathBuf> {
        // Get directory, relative to config directory if known
        match config.path.as_ref().and_then(|p| p.parent()) {
            Some(config_dir) => Some(config_dir.join(config.server.directory.as_ref()?)),
            None => config.server.directory.clone(),
        }
    }
}

/// Time configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Time {
    /// Sleep after number of seconds.
    pub sleep_after: u32,

    /// Minimum time in seconds to stay online when server is started.
    #[serde(alias = "minimum_online_time")]
    pub min_online_time: u32,

    /// Deep sleep after the server was frozen this many seconds (0: never, Linux only): its memory
    /// moves from compressed RAM (zswap) to the swap file on disk. About 6 MB of RAM instead of 200;
    /// waking up then takes about 3 s instead of 1.
    #[serde(default = "u32_600")]
    pub deep_sleep_after: u32,

    /// Stop a frozen server completely after it was frozen this many seconds (0: never). A stopped
    /// server uses no memory at all; the next join is a cold start instead of a thaw.
    #[serde(default)]
    pub stop_after: u32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            sleep_after: 60,
            min_online_time: 60,
            stop_after: 0,
            deep_sleep_after: 600,
        }
    }
}

/// MOTD configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Motd {
    /// MOTD when server is sleeping.
    pub sleeping: String,

    /// MOTD when server is starting.
    pub starting: String,

    /// MOTD when server is stopping.
    pub stopping: String,

    /// Use MOTD from Minecraft server once known.
    pub from_server: bool,
}

impl Default for Motd {
    fn default() -> Self {
        Self {
            sleeping: "☠ Server is sleeping\n§2☻ Join to start it up".into(),
            starting: "§2☻ Server is starting...\n§7⌛ Please wait...".into(),
            stopping: "☠ Server going to sleep...\n⌛ Please wait...".into(),
            from_server: false,
        }
    }
}

/// Join method types.
#[derive(Debug, Deserialize, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// Kick client with message.
    Kick,

    /// Hold client connection until server is ready.
    Hold,

    /// Forward connection to another host.
    Forward,

    /// Keep client in temporary fake lobby until server is ready.
    Lobby,
}

/// Join configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Join {
    /// Join methods.
    pub methods: Vec<Method>,

    /// Join kick configuration.
    #[serde(default)]
    pub kick: JoinKick,

    /// Join hold configuration.
    #[serde(default)]
    pub hold: JoinHold,

    /// Join forward configuration.
    #[serde(default)]
    pub forward: JoinForward,

    /// Join lobby configuration.
    #[serde(default)]
    pub lobby: JoinLobby,
}

impl Default for Join {
    fn default() -> Self {
        Self {
            methods: vec![Method::Hold, Method::Kick],
            kick: Default::default(),
            hold: Default::default(),
            forward: Default::default(),
            lobby: Default::default(),
        }
    }
}

/// Join kick configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct JoinKick {
    /// Kick message when server is starting.
    pub starting: String,

    /// Kick message when server is stopping.
    pub stopping: String,
}

impl Default for JoinKick {
    fn default() -> Self {
        Self {
            starting: "Server is starting... §c♥§r\n\nThis may take some time.\n\nPlease try to reconnect in a minute.".into(),
            stopping: "Server is going to sleep... §7☠§r\n\nPlease try to reconnect in a minute to wake it again.".into(),
        }
    }
}

/// Join hold configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct JoinHold {
    /// Hold client for number of seconds on connect while server starts.
    pub timeout: u32,
}

impl Default for JoinHold {
    fn default() -> Self {
        Self { timeout: 25 }
    }
}

/// Join forward configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct JoinForward {
    /// IP and port to forward to.
    #[serde(deserialize_with = "to_socket_addrs")]
    pub address: SocketAddr,

    /// Add HAProxy v2 header to proxied connections.
    #[serde(default)]
    pub send_proxy_v2: bool,
}

impl Default for JoinForward {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:25565".parse().unwrap(),
            send_proxy_v2: false,
        }
    }
}
/// Join lobby configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct JoinLobby {
    /// Hold client in lobby for number of seconds on connect while server starts.
    pub timeout: u32,

    /// Message banner in lobby shown to client.
    pub message: String,

    /// Sound effect to play when server is ready.
    pub ready_sound: Option<String>,
}

impl Default for JoinLobby {
    fn default() -> Self {
        Self {
            timeout: 10 * 60,
            message: "§2Server is starting\n§7⌛ Please wait...".into(),
            ready_sound: Some("block.note_block.chime".into()),
        }
    }
}

/// Lockout configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Lockout {
    /// Enable to prevent everybody from connecting through Blink. Instantly kicks player.
    pub enabled: bool,

    /// Kick players with following message.
    pub message: String,
}

impl Default for Lockout {
    fn default() -> Self {
        Self {
            enabled: false,
            message: "Server is closed §7☠§r\n\nPlease come back another time.".into(),
        }
    }
}

/// RCON configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Rcon {
    /// Enable sleeping server through RCON.
    pub enabled: bool,

    /// Server RCON port.
    pub port: u16,

    /// Server RCON password.
    pub password: String,

    /// Randomize server RCON password on each start.
    pub randomize_password: bool,

    /// Add HAProxy v2 header to RCON connections.
    pub send_proxy_v2: bool,
}

impl Default for Rcon {
    fn default() -> Self {
        Self {
            enabled: cfg!(windows),
            port: 25575,
            password: "".into(),
            randomize_password: true,
            send_proxy_v2: false,
        }
    }
}

/// Advanced configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Advanced {
    /// Rewrite server.properties.
    pub rewrite_server_properties: bool,

    /// Threads for passing on the players' traffic. 1 uses the least memory and is plenty for a
    /// normal server; give a server with hundreds of players a few more.
    pub threads: usize,

    /// Anonymous usage count: once a day Blink downloads a 1 byte file from Blink's GitHub
    /// releases, and GitHub counts the downloads. Nothing is sent. Also off with DO_NOT_TRACK=1.
    pub usage_stats: bool,
}

impl Default for Advanced {
    fn default() -> Self {
        Self {
            rewrite_server_properties: true,
            threads: 1,
            usage_stats: true,
        }
    }
}

/// Config configuration.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ConfigConfig {
    /// The Blink version this config was written for.
    pub version: Option<String>,
}

fn option_pathbuf_dot() -> Option<PathBuf> {
    Some(".".into())
}

fn server_address_default() -> SocketAddr {
    "127.0.0.1:25566".parse().unwrap()
}

fn u32_600() -> u32 {
    600
}

fn u32_300() -> u32 {
    300
}

fn u32_150() -> u32 {
    150
}

fn bool_true() -> bool {
    true
}
