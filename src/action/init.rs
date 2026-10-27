//! `blink init`: write a blink.toml for the server in this folder. Works on every OS, needs nothing
//! but the server jar: the Minecraft version, protocol and MOTD are read from the server itself.

use std::fs;
use std::path::{Path, PathBuf};

use clap::ArgMatches;

use crate::util::error::{quit_error_msg, ErrorHintsBuilder};

pub fn invoke(matches: &ArgMatches) {
    let path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).to_path_buf();
    let force = matches.get_flag("force");
    if path.exists() && !force {
        quit_error_msg(
            format!("{} already exists", path.display()),
            ErrorHintsBuilder::default().add_info("Use --force to write it again".into()).build().unwrap(),
        );
    }

    let jar = find_jar(&dir).unwrap_or_else(|| {
        quit_error_msg(
            "No Paper jar in this folder (Paper, Purpur and other Paper forks work)",
            ErrorHintsBuilder::default().build().unwrap(),
        )
    });
    let java_path = crate::launcher::find_java(&Default::default());
    let (brand, version, protocol) = server_version(&dir, &jar, &java_path).unwrap_or_else(|| {
        quit_error_msg(
            format!("Could not read the Minecraft version from {jar}; is Java installed? ({})", java_path.display()),
            ErrorHintsBuilder::default().build().unwrap(),
        )
    });

    // server.properties: our port and MOTD. Blink moves the server to another port in there, so keep
    // the original once.
    let props_path = dir.join("server.properties");
    let original = dir.join("server.properties.before-blink");
    if props_path.is_file() && !original.exists() {
        let _ = fs::copy(&props_path, &original);
    }
    let props = fs::read_to_string(if original.is_file() { &original } else { &props_path }).unwrap_or_default();
    let prop = |key: &str| {
        props.lines().find_map(|l| l.strip_prefix(&format!("{key}=")).map(unescape))
    };
    let port: u16 = prop("server-port").and_then(|p| p.trim().parse().ok()).unwrap_or(25565);
    let motd = prop("motd").map(|m| m.lines().next().unwrap_or("").to_string()).filter(|m| !m.is_empty());
    let motd = motd.unwrap_or_else(|| "A Minecraft Server".into());

    let heap = matches.get_one::<String>("heap").cloned().unwrap_or_else(default_heap);
    let sleep: u32 = *matches.get_one::<u32>("sleep").unwrap();
    let threads: usize = *matches.get_one::<usize>("threads").unwrap();
    let java = java_path;
    let java = if java.is_absolute() { format!(" JAVA={}", java.display()) } else { String::new() };
    // real player IPs need Paper's proxy-protocol setting, which exists after the first start
    let proxy = dir.join("config").join("paper-global.yml").is_file();

    let toml = format!(
        r#"# Blink: saves and freezes the server {sleep} seconds after the last player left, wakes it up
# in 0.1 s when someone joins. All settings: https://github.com/kylianxdev/blinkmc/blob/main/res/blink.toml
[public]
address = "0.0.0.0:{port}"
version = {version_name}
protocol = {protocol}

[server]
# Blink moves the real server here (server-ip and server-port in server.properties)
address = "127.0.0.1:{inner}"
directory = "."
# Blink's built in Paper launcher: Paper with Java's AOT cache, starts in about 4 s
command = {command}
freeze_process = true
wake_on_start = true
save_before_freeze = true
# with the whitelist on in server.properties, only whitelisted players can wake the server
wake_whitelist = true
wake_known_players = false
# pass on the players' real IP addresses{proxy_note}
send_proxy_v2 = {proxy}

[time]
sleep_after = {sleep}
minimum_online_time = {min_online}
# after a day frozen, stop completely (no memory at all; the next join is a 4 s cold start)
stop_after = 86400

[motd]
sleeping = {sleeping}
starting = {starting}
stopping = {stopping}
from_server = true

[join]
methods = ["hold", "kick"]

[join.hold]
timeout = 40

[join.kick]
starting = "§aThe server is waking up!§r\n\n§7Try again in §e10 seconds§7."
stopping = "§7The server is going to sleep...§r\n\n§7Try again in §e10 seconds§7."

[advanced]
threads = {threads}
usage_stats = true

[config]
version = "0.2.11"
"#,
        version_name = quote(&format!("{brand} {version}")),
        inner = port as u32 + 1,
        command = quote(&format!("blink-paper HEAP={heap} JAR={jar}{java}")),
        proxy_note = if proxy { "" } else { " (after the first start: run blink init --force again)" },
        min_online = sleep.min(60),
        sleeping = quote(&format!("{motd}\n§7💤 sleeping §8• §ajoin to wake it up")),
        starting = quote(&format!("{motd}\n§e⌛ waking up...")),
        stopping = quote(&format!("{motd}\n§7💤 going to sleep...")),
    );
    if let Err(err) = fs::write(&path, toml) {
        quit_error_msg(format!("Could not write {}: {err}", path.display()), ErrorHintsBuilder::default().build().unwrap());
    }

    eprintln!("Wrote {} for {brand} {version} on port {port}.", path.display());
    eprintln!("Start it with:  blink start");
}

/// The server jar: paper.jar, or any jar with Paper's launcher lists.
fn find_jar(dir: &Path) -> Option<String> {
    let is_paper = |p: &Path| crate::zip::read_file(p, "META-INF/versions.list").is_some();
    if is_paper(&dir.join("paper.jar")) {
        return Some("paper.jar".into());
    }
    let mut jars: Vec<_> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    jars.sort();
    jars.into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "jar") && is_paper(p))
        .find_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Brand, Minecraft version and protocol, from the server jar that Paper's jar unpacks into
/// versions/ (unpacked here first if the server never ran).
fn server_version(dir: &Path, jar: &str, java: &Path) -> Option<(String, String, u32)> {
    let list = String::from_utf8(crate::zip::read_file(&dir.join(jar), "META-INF/versions.list")?).ok()?;
    let inner = dir.join("versions").join(list.split('\t').nth(2)?.trim());
    if !inner.is_file() {
        eprintln!("Unpacking {jar} to read its version...");
        std::process::Command::new(java)
            .args(["-Dpaperclip.patchonly=true", "-jar", jar])
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .status()
            .ok()?;
    }
    let inner = fs::read(inner).ok()?;
    let info: serde_json::Value = serde_json::from_slice(&crate::zip::read(&inner, "version.json")?).ok()?;
    let manifest = String::from_utf8(crate::zip::read(&inner, "META-INF/MANIFEST.MF").unwrap_or_default()).unwrap_or_default();
    let brand = manifest
        .lines()
        .find_map(|l| l.strip_prefix("Brand-Name: "))
        .unwrap_or("Paper")
        .trim()
        .to_string();
    Some((brand, info["name"].as_str()?.to_string(), info["protocol_version"].as_u64()? as u32))
}

/// Java .properties escapes: \uXXXX, \n, \: and friends.
fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// A TOML string.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

/// 60% of the RAM, at most 8G, at least 2G (4G when we can't tell).
fn default_heap() -> String {
    let total_gb = fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|m| m.lines().find_map(|l| l.strip_prefix("MemTotal:").map(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())))
        .flatten()
        .map(|kb| kb / 1024 / 1024);
    match total_gb {
        Some(gb) => format!("{}G", (gb * 6 / 10).clamp(2, 8)),
        None => "4G".into(),
    }
}
