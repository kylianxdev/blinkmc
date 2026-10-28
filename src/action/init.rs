//! `blink init`: write a blink.toml for the server in this folder. Works on every OS, needs nothing
//! but the server itself: Paper (and its forks), Fabric, Quilt, Forge, NeoForge or vanilla. The
//! Minecraft version, protocol and MOTD are read from the server's own files.

use std::fs;
use std::path::{Path, PathBuf};

use clap::ArgMatches;

use crate::util::error::{quit_error_msg, ErrorHintsBuilder};

/// The kind of server in the folder, and how to start it.
enum Kind {
    /// Paper or a fork: Blink's own launcher with the AOT cache.
    Paper { jar: String },
    /// Fabric, Quilt, vanilla or another server jar: `java -jar`.
    Jar { jar: String, name: &'static str },
    /// Forge or NeoForge: the java line from their run script (its @argument files).
    Forge { args: Vec<String> },
}

pub fn invoke(matches: &ArgMatches) {
    let path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).to_path_buf();
    let force = matches.get_flag("force");
    if path.exists() && !force {
        quit_error_msg(
            format!("{} already exists", path.display()),
            ErrorHintsBuilder::default().add_info("Use --force to write it again".into()).verbose(false).build().unwrap(),
        );
    }

    let kind = find_server(&dir).unwrap_or_else(|| {
        quit_error_msg(
            format!("No Minecraft server in {}", fs::canonicalize(&dir).unwrap_or(dir.clone()).display()),
            ErrorHintsBuilder::default()
                .add_info("Run blink init in the folder with your server: paper.jar (or Purpur), a Fabric or vanilla server jar, or Forge's run script".into())
                .verbose(false)
                .build()
                .unwrap(),
        )
    });
    let java_path = crate::launcher::find_java(&Default::default());
    let (brand, version, protocol) = match &kind {
        Kind::Paper { jar } => server_version(&dir, jar, &java_path).unwrap_or_else(|| {
            quit_error_msg(
                format!("Could not read the Minecraft version from {jar}; is Java installed? ({})", java_path.display()),
                ErrorHintsBuilder::default()
                    .add_info("Install Java 26 (for example Temurin: https://adoptium.net) and run blink init again".into())
                    .verbose(false)
                    .build()
                    .unwrap(),
            )
        }),
        Kind::Jar { name, .. } => vanilla_version(&dir).map_or_else(
            || (name.to_string(), String::new(), crate::proto::PROTO_DEFAULT_PROTOCOL),
            |(version, protocol)| (name.to_string(), version, protocol),
        ),
        Kind::Forge { args } => {
            let name = if args.iter().any(|a| a.contains("neoforge")) { "NeoForge" } else { "Forge" };
            vanilla_version(&dir).map_or_else(
                || (name.to_string(), String::new(), crate::proto::PROTO_DEFAULT_PROTOCOL),
                |(version, protocol)| (name.to_string(), version, protocol),
            )
        }
    };

    // server.properties: our port and MOTD. Blink moves the server to another port in there, so keep
    // the original once (blink restore puts its values back).
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

    let (command, command_note, proxy) = match &kind {
        Kind::Paper { jar } => {
            // the Java it found, so a later JAVA_HOME or update can't pick another one by surprise
            let java = if java_path.is_absolute() { format!(" {}", setting("JAVA", &java_path.display().to_string())) } else { String::new() };
            (
                format!("blink-paper HEAP={heap} {}{java}", setting("JAR", jar)),
                "# Blink's built in Paper launcher: Paper with Java's AOT cache (Java 26), starts in about 4 s",
                true,
            )
        }
        Kind::Jar { jar, .. } => (
            format!("java -Xms256M -Xmx{heap} -jar {} nogui", shlex::try_quote(jar).unwrap_or_default()),
            "# how the server starts; Blink freezes this process and everything it starts",
            false,
        ),
        Kind::Forge { args } => {
            // the heap lives in user_jvm_args.txt; add one only if it sets none
            let jvm_args = fs::read_to_string(dir.join("user_jvm_args.txt")).unwrap_or_default();
            let has_heap = jvm_args.lines().any(|l| !l.trim_start().starts_with('#') && l.contains("-Xmx"));
            let heap = if has_heap { String::new() } else { format!(" -Xms256M -Xmx{heap}") };
            let args: Vec<String> = args.iter().map(|a| shlex::try_quote(a).unwrap_or_default().to_string()).collect();
            (
                format!("java{heap} {} nogui", args.join(" ")),
                "# Forge's own start (run.sh / run.bat), with java started directly so Blink can freeze it",
                false,
            )
        }
    };

    let toml = format!(
        r#"# Blink: saves and freezes the server {sleep} seconds after the last player left, and wakes it up
# when someone joins (in the world in about a second). All settings, explained:
# https://github.com/kylianxdev/blinkmc/blob/main/res/blink.toml
[public]
address = "0.0.0.0:{port}"
version = {version_name}
protocol = {protocol}

[server]
# Blink moves the real server here (server-ip and server-port in server.properties)
address = "127.0.0.1:{inner}"
directory = "."
{command_note}
command = {command}
freeze_process = true
wake_on_start = true
save_before_freeze = true
# with the whitelist on in server.properties, only whitelisted players can wake the server
wake_whitelist = true
wake_known_players = false
# pass on the players' real IP addresses (PROXY protocol){proxy_note}
send_proxy_v2 = {proxy}

[time]
sleep_after = {sleep}
minimum_online_time = {min_online}
# after 10 minutes frozen, its memory moves to the swap file: a few MB of RAM, waking up takes about 3 s (Linux)
deep_sleep_after = 600
# after a day frozen, stop completely (no memory at all; the next join is a cold start)
stop_after = 86400

[motd]
sleeping = {sleeping}
starting = {starting}
stopping = {stopping}
# borrow the server's own icon for the server list
from_server = true

[join]
methods = ["hold", "kick"]

[join.hold]
# shorter than the client's own 30 s timeout, so a slow start shows the message below instead
timeout = 25

[join.kick]
starting = "§aThe server is waking up!§r\n\n§7Try again in §e10 seconds§7."
stopping = "§7The server is going to sleep...§r\n\n§7Try again in §e10 seconds§7."

[advanced]
threads = {threads}
# one download a day from GitHub so we can count servers; nothing is sent (false turns it off)
usage_stats = true

[config]
version = "{config_version}"
"#,
        version_name = quote(format!("{brand} {version}").trim()),
        // well away from the public port: in a network the next ports are often other servers
        inner = if port <= 55535 { port as u32 + 10000 } else { port as u32 - 10000 },
        command = quote(&command),
        proxy_note = if proxy { "" } else { "; only with a PROXY protocol mod or plugin" },
        min_online = sleep.min(60),
        sleeping = quote(&format!("{motd}\n§7💤 sleeping §8• §ajoin to wake it up")),
        starting = quote(&format!("{motd}\n§e⌛ waking up...")),
        stopping = quote(&format!("{motd}\n§7💤 going to sleep...")),
        config_version = crate_version!(),
    );
    if let Err(err) = fs::write(&path, toml) {
        quit_error_msg(format!("Could not write {}: {err}", path.display()), ErrorHintsBuilder::default().build().unwrap());
    }

    eprintln!("Wrote {} for {} on port {port}.", path.display(), format!("{brand} {version}").trim());
    if !fs::read_to_string(dir.join("eula.txt")).is_ok_and(|t| t.lines().any(|l| l.trim() == "eula=true")) {
        eprintln!("Before the first start: accept the Minecraft EULA (https://aka.ms/MinecraftEULA) with eula=true in eula.txt.");
    }
    eprintln!("Start it with:  blink start");
}

/// What kind of server is in the folder: Paper first, then Forge's run script, then a server jar.
fn find_server(dir: &Path) -> Option<Kind> {
    if let Some(jar) = find_paper(dir) {
        return Some(Kind::Paper { jar });
    }
    if let Some(args) = forge_args(dir) {
        return Some(Kind::Forge { args });
    }
    let mut jars: Vec<String> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".jar"))
        .collect();
    jars.sort();
    for (prefix, name) in [("fabric-server", "Fabric"), ("quilt-server", "Quilt"), ("server.jar", "Minecraft"), ("minecraft_server", "Minecraft")] {
        if let Some(jar) = jars.iter().find(|j| j.starts_with(prefix)) {
            return Some(Kind::Jar { jar: jar.clone(), name });
        }
    }
    // one jar and nothing else: that's the server
    match jars.as_slice() {
        [jar] => Some(Kind::Jar { jar: jar.clone(), name: "Minecraft" }),
        _ => None,
    }
}

/// The server jar: paper.jar, or any jar with Paper's launcher lists.
fn find_paper(dir: &Path) -> Option<String> {
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

/// Forge and NeoForge start with `java @user_jvm_args.txt @libraries/.../unix_args.txt "$@"` in
/// run.sh (win_args.txt and %* in run.bat). The script itself can't be frozen reliably, so Blink
/// starts that java line directly.
fn forge_args(dir: &Path) -> Option<Vec<String>> {
    let script = fs::read_to_string(dir.join(if cfg!(windows) { "run.bat" } else { "run.sh" })).ok()?;
    let line = script.lines().map(str::trim).find(|l| l.starts_with("java ") && l.contains("@libraries"))?;
    let args = if cfg!(windows) { line.split_whitespace().map(String::from).collect() } else { shlex::split(line)? };
    Some(args.into_iter().skip(1).filter(|a| a != "$@" && a != "%*" && a != "nogui").collect())
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

/// Minecraft version and protocol from a vanilla server jar in the folder (Fabric and Quilt keep
/// one as server.jar). None if there is none: the server list then shows what the server says once
/// it ran.
fn vanilla_version(dir: &Path) -> Option<(String, u32)> {
    let mut jars: Vec<_> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "jar")).collect();
    jars.sort();
    jars.iter().find_map(|jar| {
        let info: serde_json::Value = serde_json::from_slice(&crate::zip::read_file(jar, "version.json")?).ok()?;
        Some((info["name"].as_str()?.to_string(), info["protocol_version"].as_u64()? as u32))
    })
}

/// A launcher setting as KEY=VALUE, quoted when the value has spaces ("C:/Program Files/...").
/// Forward slashes: the command line treats a backslash as an escape (Windows accepts both).
fn setting(key: &str, value: &str) -> String {
    let value = value.replace('\\', "/");
    if value.contains(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
        format!("{key}={}", shlex::try_quote(&value).unwrap_or_default())
    } else {
        format!("{key}={value}")
    }
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
    match total_ram_gb() {
        Some(gb) => format!("{}G", (gb * 6 / 10).clamp(2, 8)),
        None => "4G".into(),
    }
}

/// The machine's RAM in GB.
#[cfg(target_os = "linux")]
fn total_ram_gb() -> Option<u64> {
    fs::read_to_string("/proc/meminfo")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:").map(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok()))
        .flatten()
        .map(|kb| kb / 1024 / 1024)
}

/// The machine's RAM in GB.
#[cfg(target_os = "macos")]
fn total_ram_gb() -> Option<u64> {
    let out = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok().map(|b| b >> 30)
}

/// The machine's RAM in GB.
#[cfg(windows)]
fn total_ram_gb() -> Option<u64> {
    use winapi::um::sysinfoapi::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.ullTotalPhys >> 30)
}

/// The machine's RAM in GB.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn total_ram_gb() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_path_with_spaces_stays_one_argument() {
        let command = format!("blink-paper HEAP=4G JAR=paper.jar {}", setting("JAVA", r"C:\Program Files\Eclipse Adoptium\jdk-26\bin\java.exe"));
        let args = shlex::split(&command).unwrap();
        let settings = crate::launcher::settings(&args[1..]);
        assert_eq!(settings["JAVA"], "C:/Program Files/Eclipse Adoptium/jdk-26/bin/java.exe");
        assert_eq!(settings["JAR"], "paper.jar");
    }

    #[test]
    fn plain_values_are_not_quoted() {
        assert_eq!(setting("JAVA", "/usr/lib/jvm/java-26/bin/java"), "JAVA=/usr/lib/jvm/java-26/bin/java");
    }

    #[test]
    fn forge_run_script() {
        let dir = std::env::temp_dir().join(format!("blink-forge-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (file, line) = if cfg!(windows) {
            ("run.bat", "java @user_jvm_args.txt @libraries/net/neoforged/neoforge/21.1.77/win_args.txt %*")
        } else {
            ("run.sh", "java @user_jvm_args.txt @libraries/net/neoforged/neoforge/21.1.77/unix_args.txt \"$@\"")
        };
        fs::write(dir.join(file), format!("#!/usr/bin/env sh\n# comment\n{line}\npause\n")).unwrap();
        let args = forge_args(&dir).unwrap();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "@user_jvm_args.txt");
        assert!(args[1].starts_with("@libraries/net/neoforged"));
        let _ = fs::remove_dir_all(&dir);
    }
}
