//! Blink's Paper launcher (`command = "blink-paper ..."` in blink.toml). Works the same on Linux,
//! macOS and Windows, and needs nothing but Java.
//!
//! It starts Paper as fast as possible (about 4 s instead of 13 s):
//!  1. Paper is started directly instead of through paperclip, so all of its classes come from the
//!     normal class path and fit in Java's AOT cache.
//!  2. The AOT cache (paper.aot) holds the loaded and linked classes, lambdas and method profiles.
//!     It needs Java 26: on Java 25 Minecraft's MethodType table is too big for the cache
//!     ("Cannot archive the sub-graph ... size 262160"), with AOTStreamableObjects it fits.
//!  3. No -XX:+AlwaysPreTouch: touching the whole heap up front costs seconds.
//!
//! The cache belongs to one Paper build and one Java build. After an update the first start is a
//! training run (slow), and the start after it builds the cache.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAIN: &str = "org.bukkit.craftbukkit.Main";

/// The Java command for a Paper start, after preparing the folder (unpacking, cache).
pub struct Launch {
    pub java: PathBuf,
    pub args: Vec<String>,
}

/// Settings from `blink-paper KEY=VALUE ...`.
pub fn settings(args: &[String]) -> HashMap<String, String> {
    args.iter()
        .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

/// Find Java: JAVA from the settings, then JAVA_HOME, then the newest JDK in the usual places, then
/// `java` on the PATH.
pub fn find_java(settings: &HashMap<String, String>) -> PathBuf {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    if let Some(java) = settings.get("JAVA") {
        return PathBuf::from(java);
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let java = Path::new(&home).join("bin").join(exe);
        if java.is_file() {
            return java;
        }
    }
    let roots = [
        "/usr/lib/jvm",
        "/Library/Java/JavaVirtualMachines",
        "C:\\Program Files\\Java",
        "C:\\Program Files\\Eclipse Adoptium",
        "C:\\Program Files\\Microsoft",
    ];
    let mut found: Vec<(u32, PathBuf)> = vec![];
    for root in roots {
        let Ok(dirs) = fs::read_dir(root) else { continue };
        for dir in dirs.flatten() {
            let name = dir.file_name().to_string_lossy().to_string();
            // jdk-26, java-26-openjdk-amd64, temurin-26.jdk, jdk-26.0.2+10 ...
            let version = name
                .split(|c: char| !c.is_ascii_digit())
                .find(|p| !p.is_empty())
                .and_then(|p| p.parse::<u32>().ok())
                .unwrap_or(0);
            for java in [dir.path().join("bin").join(exe), dir.path().join("Contents/Home/bin").join(exe)] {
                if java.is_file() {
                    found.push((version, java));
                }
            }
        }
    }
    found.sort();
    found.pop().map(|(_, java)| java).unwrap_or_else(|| PathBuf::from(exe))
}

/// Prepare the server folder and return the Java command to start Paper with.
pub fn prepare(dir: &Path, args: &[String], proxy_protocol: bool) -> Result<Launch, String> {
    let settings = settings(args);
    let get = |key: &str, default: &str| settings.get(key).cloned().unwrap_or_else(|| default.to_string());
    let java = find_java(&settings);
    let jar = get("JAR", "paper.jar");
    let heap = get("HEAP", "8G");
    let heap_min = get("HEAP_MIN", "256M");

    // Aikar's flags, without AlwaysPreTouch (slow start) and ParallelRefProcEnabled (deprecated in
    // 26). Start small and give memory back when it is quiet: G1 collects after 15 s without other
    // GCs and returns what it doesn't need, so an empty server is small by the time it is frozen.
    let mut flags: Vec<String> = format!(
        "-Xms{heap_min} -Xmx{heap} -XX:G1PeriodicGCInterval=15000 -XX:MinHeapFreeRatio=15 \
         -XX:MaxHeapFreeRatio=40 -XX:+DisableExplicitGC -XX:+PerfDisableSharedMem \
         -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1HeapRegionSize=8M -XX:G1HeapWastePercent=5 \
         -XX:G1MaxNewSizePercent=40 -XX:G1MixedGCCountTarget=4 -XX:G1MixedGCLiveThresholdPercent=90 \
         -XX:G1NewSizePercent=30 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:G1ReservePercent=20 \
         -XX:InitiatingHeapOccupancyPercent=15 -XX:MaxGCPauseMillis=200 -XX:MaxTenuringThreshold=1 \
         -XX:SurvivorRatio=32"
    )
    .split_whitespace()
    .map(String::from)
    .collect();
    if cfg!(target_os = "linux") {
        flags.push("-XX:+UseTransparentHugePages".into());
    }
    if let Some(extra) = settings.get("JAVA_FLAGS") {
        flags.extend(extra.split_whitespace().map(String::from));
    }

    // A frozen server sees a gap of minutes or days when it wakes up. Paper's watchdog would call
    // that a hang and kill it, and restart-on-crash would start a second server outside Blink.
    let spigot = dir.join("spigot.yml");
    if !spigot.exists() {
        // a new server: write the two settings now, Spigot adds the rest on its first start
        let _ = fs::write(&spigot, "settings:\n  timeout-time: 1728000\n  restart-on-crash: false\n");
    }
    set_yaml(&spigot, &[("  timeout-time:", "1728000"), ("  restart-on-crash:", "false")]);
    let mut global = vec![("  early-warning-delay:", "1728000000")];
    if proxy_protocol {
        // Blink passes on the players' real IP addresses with the PROXY protocol
        global.push(("  proxy-protocol:", "true"));
    }
    set_yaml(&dir.join("config").join("paper-global.yml"), &global);

    // New Paper jar or new Java: unpack Paper, write the class path again, throw the cache away
    let stamp = format!("{} {}", file_stamp(&dir.join(&jar)), file_stamp(&fs::canonicalize(&java).unwrap_or(java.clone())));
    if fs::read_to_string(dir.join("paper.aot.stamp")).ok().as_deref() != Some(stamp.as_str()) {
        info!(target: "lazymc", "New Paper or Java: unpacking Paper, the first start is a training run for the cache");
        let ok = Command::new(&java)
            .args(["-Dpaperclip.patchonly=true", "-jar", &jar])
            .current_dir(dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return Err(format!("could not run {} -jar {jar}; is Java 26 installed? Set JAVA= in the command", java.display()));
        }
        fs::write(dir.join("classpath.txt"), class_path(&dir.join(&jar))?).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(dir.join("paper.aot"));
        let _ = fs::remove_file(dir.join("paper.aot.config"));
        fs::write(dir.join("paper.aot.stamp"), &stamp).map_err(|e| e.to_string())?;
    }
    let class_path = fs::read_to_string(dir.join("classpath.txt")).map_err(|e| e.to_string())?;

    // A finished training run left paper.aot.config: build the cache from it now (a few seconds)
    let config = dir.join("paper.aot.config");
    let cache = dir.join("paper.aot");
    if has_data(&config) && !has_data(&cache) {
        info!(target: "lazymc", "Building the AOT cache from the training run");
        let ok = Command::new(&java)
            .args(&flags)
            .args(["-XX:+UnlockDiagnosticVMOptions", "-XX:+AOTStreamableObjects", "-XX:AOTMode=create"])
            .args(["-XX:AOTConfiguration=paper.aot.config", "-XX:AOTCache=paper.aot", "-cp", &class_path])
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            warn!(target: "lazymc", "Could not build the AOT cache (needs Java 26); starting without it");
            let _ = fs::remove_file(&cache);
        }
        let _ = fs::remove_file(&config);
    }

    let mut args = flags;
    if has_data(&cache) {
        args.push("-XX:AOTCache=paper.aot".into());
    } else {
        // Training run: Java writes down what Paper uses when it stops; the next start builds the cache
        args.extend(["-XX:AOTMode=record".into(), "-XX:AOTConfiguration=paper.aot.config".into()]);
    }
    args.extend(["-cp".into(), class_path, MAIN.into(), "--nogui".into()]);
    Ok(Launch { java, args })
}

/// The class path from Paper's launcher lists: the server jar and its libraries.
fn class_path(jar: &Path) -> Result<String, String> {
    let list = |name: &str| {
        crate::zip::read_file(jar, name)
            .map(|d| String::from_utf8_lossy(&d).to_string())
            .ok_or(format!("{} is not a Paper jar ({name} missing)", jar.display()))
    };
    let version = list("META-INF/versions.list")?;
    let version = version.split('\t').nth(2).ok_or("unexpected versions.list")?.trim().to_string();
    let mut paths = vec![format!("versions/{version}")];
    for line in list("META-INF/libraries.list")?.lines().filter(|l| !l.trim().is_empty()) {
        if let Some(path) = line.split('\t').nth(2) {
            paths.push(format!("libraries/{}", path.trim()));
        }
    }
    Ok(paths.join(if cfg!(windows) { ";" } else { ":" }))
}

/// Set `key: value` on lines that start with the key (with its indentation), if the file exists.
fn set_yaml(path: &Path, settings: &[(&str, &str)]) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let changed: Vec<String> = text
        .lines()
        .map(|line| {
            settings
                .iter()
                .find(|(key, _)| line.starts_with(key))
                .map(|(key, value)| format!("{key} {value}"))
                .unwrap_or_else(|| line.to_string())
        })
        .collect();
    let mut changed = changed.join("\n");
    if text.ends_with('\n') {
        changed.push('\n');
    }
    if changed != text {
        let _ = fs::write(path, changed);
    }
}

fn file_stamp(path: &Path) -> String {
    fs::metadata(path)
        .map(|m| {
            let modified = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            format!("{}-{}", m.len(), modified)
        })
        .unwrap_or_default()
}

fn has_data(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
}
