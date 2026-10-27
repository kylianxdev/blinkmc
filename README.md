<p align="center">
  <img src="docs/assets/banner.png" alt="Blink: your Minecraft server sleeps when nobody plays and wakes up in 0.1 seconds" width="100%">
</p>

<p align="center">
  <a href="https://kylianxdev.github.io/blinkmc/"><b>Website</b></a> ·
  <a href="#7-install">Install</a> ·
  <a href="#4-results">Results</a> ·
  <a href="https://github.com/kylianxdev/lazymc/tree/blink">lazymc fork</a>
</p>

<p align="center">
  <img alt="Paper 26+" src="https://img.shields.io/badge/Paper-26%2B-36c46b?style=flat-square">
  <img alt="Java 26" src="https://img.shields.io/badge/Java-26-f5c542?style=flat-square">
  <img alt="Wake up 0.1 s" src="https://img.shields.io/badge/wake%20up-0.1%20s-6c8cff?style=flat-square">
  <img alt="MIT" src="https://img.shields.io/badge/license-MIT-9aa5d6?style=flat-square">
</p>

# Blink

**Blink makes your Minecraft server sleep when nobody plays and wake up in 0.1 seconds when someone
joins.** The sleeping server gives most of its memory back to the machine, the server list keeps
showing your MOTD and icon, and players don't install anything. Stock Paper, stock Java, no mods.

| | Stock Paper | With Blink |
|---|---|---|
| Join on a sleeping server until the real server takes over | not possible | **0.1 to 0.4 s** |
| Cold start (after a reboot or an update) | 13.2 s | **5.0 s** |
| Memory of a server nobody plays on | 3.0 GB in RAM, 8 GB reserved | **0.9 GB**, compressed |
| Server list (MOTD, icon) while asleep | | works |

This README is written as a short paper: what we wanted, how we got there, what failed on the way,
and how to do it yourself. If you just want it running, jump to [Install](#7-install).

Kylian de Groot, October 2026

---

## Abstract

A small private Minecraft server is empty most of the day, yet it holds on to gigabytes of memory.
Stopping it when nobody plays is easy; starting it again so fast that a player does not notice is
the hard part. Blink combines a sleep proxy (**lazymc**, with our patches) with two ways back:

1. **Freeze.** The JVM is paused instead of stopped. Linux moves its memory into compressed RAM
   (zswap) and swap, which frees about 70% of it. A join continues the paused process:
   **0.1 to 0.4 s**.
2. **Cold start.** For reboots and updates. Paper is started without its paperclip launcher so
   **Java's AOT cache** (Project Leyden) can hold all of its classes. Java 26's streamable AOT
   objects are needed, because Minecraft's method type table is too big for the older format.
   **13.2 s down to 5.0 s.**

Everything is a shell script, a config file and a small lazymc patch. Nothing in Paper or
Minecraft is changed.

## 1. Intent

The server in this write-up is a survival world for a handful of friends on a VPS with 6 cores and
11.6 GB of RAM. Next to Minecraft it runs a live web map (BlueMap) and a stats website. Most of the
day nobody is online, but the server still reserved 8 GB and left about 1.7 GB for everything else.

The goals were:

1. **Asleep when nobody plays**, and the memory given back.
2. **Invisible to players.** The server stays in everyone's server list with its normal MOTD and
   icon. Joining a sleeping server just works.
3. **Fast.** First target: under 5 seconds. Then: **under 1 second.**
4. **Nothing else breaks.** Web map, backups and chat announcements keep working.
5. **No forks** of Paper, no mods, nothing on the client.

## 2. Background

**lazymc** ([timvisee/lazymc](https://github.com/timvisee/lazymc)) listens on the public port in
front of the real server. While the server sleeps it answers server list pings itself. On a login it
wakes the server and *holds* the client: the connection stays open until the server is ready, then
lazymc relays it. After a configurable idle time it puts the server to sleep again, either by
stopping it or by freezing it with `SIGSTOP`.

**Paper's start** consists of: starting the JVM, the paperclip launcher (which checks and unpacks the
real server jar), Minecraft's bootstrap (registering every block, item, entity, codec and data
fixer), loading data packs, loading the worlds and enabling plugins. Most of it is class loading and
code that runs exactly once: the worst case for a JIT compiler and the best case for an ahead of
time cache.

**The AOT cache** ([JEP 483](https://openjdk.org/jeps/483), [514](https://openjdk.org/jeps/514),
[515](https://openjdk.org/jeps/515), [516](https://openjdk.org/jeps/516)) is built from a training
run. Later runs map it and start with classes already loaded and linked, lambdas already created and
method profiles already present.

## 3. Method

### 3.1 Measuring

`tools/measure-startup.sh` stops the server, starts it with a given script and measures the wall time
until Paper prints `Done (...)`. `tools/status-ping.py --join` measures a join on a sleeping server:
the time from the login request until the real server answers it (its encryption request). To see
*where* time went we used Java Flight Recorder during start and `-Xlog:class+load`.

Setup: Paper 26.3 build 200, OpenJDK 25.0.4 and 26.0.2 (Ubuntu packages), 6 vCPU (Intel Haswell),
11.6 GB RAM, an 838 MB world, plugins BlueMap 5.29 and spark.

### 3.2 Baseline

`java -jar paper.jar` with Aikar's flags, an 8 GB heap and `-XX:+AlwaysPreTouch`:
**13.2 s** until "Done" (Paper: 11.5 s).

### 3.3 Cold start: Paper without paperclip

`paper.jar` is a launcher. It unpacks the real server to `versions/` and its libraries to
`libraries/`, then loads them in **its own class loader**. The AOT cache only stores classes from the
normal class path, so with paperclip almost nothing of Minecraft can be cached.

`start.sh` reads `META-INF/versions.list` and `META-INF/libraries.list` from `paper.jar`, writes
`classpath.txt` and starts `org.bukkit.craftbukkit.Main` directly. After a Paper update it first runs
`java -Dpaperclip.patchonly=true -jar paper.jar`, which unpacks the new version and exits.

We also dropped `-XX:+AlwaysPreTouch`: writing to every page of an 8 GB heap before starting costs
seconds on every start.

### 3.4 Cold start: the AOT cache on Java 25 only half works

Building the cache failed:

```
[error][aot,heap] Cannot archive the sub-graph referenced from [Ljava.util.concurrent.ConcurrentHashMap$Node; object size 262160, skipped.
[error][aot] An error has occurred while writing the shared archive file.
```

That object is the backing array of the JVM's `MethodType` intern table. Minecraft creates so many
method types (it is full of lambdas and codecs) that the array is 16 bytes over the 256 KB limit for
one archived object, and the cache file comes out empty.

With `-XX:-AOTClassLinking` the cache builds without heap objects: **7.8 s**. Class load logging
showed why not more: 14,427 of 31,475 classes still did not come from the cache, nearly all of them
lambda and hidden classes generated at run time (865 from `Blocks` alone), which can only be cached
*with* class linking.

Two smaller traps:

* `--enable-native-access=ALL-UNNAMED` (set in paperclip's manifest) breaks building a cache with
  linked classes. It only silences a warning, so it is gone.
* The one step `-XX:AOTCacheOutput` builds the cache in a child JVM when the server exits. Under tmux
  that child died with the session. Record and create as two explicit steps avoid it.

### 3.5 Cold start: Java 26 streamable objects

Java 26 can store cached heap objects in a second format ([JEP 516](https://openjdk.org/jeps/516)):
loaded as a stream of objects instead of mapped as one region. It has no 256 KB per object limit. It
is not the default for G1 yet, but a diagnostic switch turns it on:

```
-XX:+UnlockDiagnosticVMOptions -XX:+AOTStreamableObjects
```

The full cache builds (230 MB) and the start drops to **4.9 s**, **5.0 s** with the update check in
`start.sh` (which uses `stat` instead of `java -version`, itself half a second). Over 25 real cold
starts Paper reported a median of 4.35 s.

### 3.6 Under a second: freeze instead of stop

A cold start under one second is not realistic: the JVM and Minecraft's registries alone take longer.
So the fast path does not start at all. lazymc can freeze the server (`SIGSTOP`) and continue it
(`SIGCONT`). On its own that wakes instantly but keeps all memory, which defeats the point. Blink
adds:

* **Memory out of the way.** `blink-freeze-helper` notices a frozen server and writes to its cgroup's
  `memory.reclaim`. The kernel moves the heap into **zswap** (compressed with lz4, about 3.3:1 here)
  and the coldest pages to a swap file. A server with 3.0 GB in RAM goes to 0.9 GB.
* **No read ahead from swap** (`vm.page-cluster=0`). With zswap, reading 8 pages per fault only adds
  latency. Together with the next point this took a wake up from 1.2 s to 0.1 s.
* **lazymc polls every 50 ms while waking**, instead of every 2 s, and does not poll a frozen server
  at all (it cannot answer; each poll would only wait for a timeout).
* **Paper's watchdog** sees a gap of minutes or days after a thaw and kills the server as hung.
  `timeout-time` in `spigot.yml` and `early-warning-delay` in `paper-global.yml` are set to 20 days,
  and `restart-on-crash` is off (it started a second server outside lazymc).
* **Stopping a frozen server.** Paper's console reads Ctrl+C itself in raw mode, so a frozen server
  never sees it. The service sends `SIGINT` to lazymc instead, and our patch makes lazymc thaw the
  server, let Paper save and stop, and then quit.
* **`wake_on_start`.** After a reboot the server starts right away and freezes after the idle time,
  so even the first join of the day is a thaw and not a cold start.

Measured: three wake ups in a row of **0.1 s, 0.4 s and 0.4 s**, the same process every time.

### 3.7 lazymc on current Paper

lazymc 0.2.11 started the server but never noticed it was up: players were held for 40 seconds and
then kicked. Its monitor silently threw away Paper's status answer. Two known issues cause this: a
server icon in the answer ([#102](https://github.com/timvisee/lazymc/issues/102)) and a MOTD written
as a chat component. Pull request [#107](https://github.com/timvisee/lazymc/pull/107) fixes the
decoding but is not merged yet. The [Blink fork](https://github.com/kylianxdev/lazymc/tree/blink)
is lazymc `master` with #107 and the patches above.

### 3.8 Keeping the rest alive

* **Web map.** BlueMap's built in web server lives inside Paper and would freeze with it. We run
  BlueMap's CLI as a separate web server (`java -jar bluemap-cli.jar -w`, 73 MB) on the same files,
  and let the plugin write live player positions and markers to its storage.
* **Scripts that type in the console** (backups, chat announcements) check first whether the server
  is really awake. A frozen server still listens on its port, but would only run the commands after
  it wakes up.
* **A daily fresh start** happens only while the server is frozen and nobody played for an hour.

## 4. Results

| Path | Time | Notes |
|---|---|---|
| Stock Paper (paperclip, Java 25, AlwaysPreTouch) | 13.2 s | Paper: 11.5 s |
| Direct start, Java 25 AOT cache without class linking | 7.8 s | Paper: 7.5 s |
| Direct start, Java 26 full AOT cache | 5.0 s | Paper: median 4.35 s over 25 starts |
| **Thaw a frozen server (join until the real server answers)** | **0.1 to 0.4 s** | memory in zswap and swap |

| Memory | Awake, idle | Frozen |
|---|---|---|
| Minecraft service in RAM | 3.0 GB | 0.9 GB (compressed) |
| lazymc | 6 MB | 6 MB |
| BlueMap web server | 73 MB | 73 MB |

Tick performance is not affected. The AOT cache only changes how classes get loaded; hot code is JIT
compiled as before. A frozen server uses no CPU at all.

## 5. What we learned the hard way

* **Bots wake servers.** In the first days the server woke up about ten times a day, every time for
  a scanner or a bot, and then stayed up for 30 minutes. Waking needs no successful login.
  `wake_whitelist = true` lets only players from `whitelist.json` wake the server.
* **Profile before you tune.** The guess was "the world loads slowly". The world took 0.5 s; the time
  was one thread loading classes.
* **Read the line above the error.** "An error has occurred while writing the shared archive file"
  says nothing. The line before it named the exact object that did not fit.
* **A paused process has a clock problem.** Anything that measures time (watchdogs, timeouts) sees a
  frozen server as a hung server.

## 6. Limitations

* **The first start after an update is slow** (the training run, about 13 s); the cache is built when
  it stops. With `wake_on_start` that happens without anyone waiting for it.
* **Plugin classes are not cached** (they have their own class loaders). With many plugins the cold
  start gain is smaller. Thawing is not affected.
* **No watchdog for real hangs** while the timeouts are raised.
* **Disk**: 230 MB for the cache, plus a swap file.
* **Diagnostic flag.** If `AOTStreamableObjects` changes in a later Java, the cache build fails and
  `start.sh` simply starts without a cache.
* **lazymc is a fork** until #107 is released.

## 7. Install

You need Linux with systemd and tmux, Paper 26 or newer, and Java 26
(`apt install openjdk-26-jre-headless tmux` on Ubuntu 26.04).

**1. Get Blink and the fast start script**

```bash
git clone https://github.com/kylianxdev/blinkmc /opt/blinkmc
cd /path/to/your/server            # next to paper.jar
cp /opt/blinkmc/start.sh . && chmod +x start.sh
cp /opt/blinkmc/lazymc.example.toml lazymc.toml
```

Set `HEAP` in `start.sh` if 8G is not right for you.

**2. Get the patched lazymc.** Download `lazymc-blink-linux-x64` from the
[releases](https://github.com/kylianxdev/blinkmc/releases), or build it:

```bash
git clone -b blink https://github.com/kylianxdev/lazymc && cd lazymc
cargo build --release              # binary in target/release/lazymc
```

**3. Configure lazymc.** In `lazymc.toml`, set `version` and `protocol` to what your server reports
(`python3 /opt/blinkmc/tools/status-ping.py localhost` while it runs normally) and the MOTD texts.
lazymc moves the real server to `127.0.0.1:25566` in `server.properties` by itself.

**4. Tell Paper's watchdog about sleeping**

```yaml
# spigot.yml
settings:
  timeout-time: 1728000
  restart-on-crash: false
# config/paper-global.yml
watchdog:
  early-warning-delay: 1728000000
```

**5. Enable sleeping and install the service**

```bash
sudo /opt/blinkmc/setup/enable-sleep.sh        # swap file, zswap, freeze helper
sudo cp /opt/blinkmc/systemd/minecraft.service /etc/systemd/system/   # adjust paths and user
sudo systemctl daemon-reload && sudo systemctl enable --now minecraft
```

The console is still `tmux attach -t minecraft`.

**6. Test it**

```bash
python3 /opt/blinkmc/tools/status-ping.py localhost                          # server list
python3 /opt/blinkmc/tools/status-ping.py localhost --join --name YourName   # wake it
```

**Optional: BlueMap.** Download `bluemap-<version>-cli.jar`, copy your plugin's `maps/` and
`storages/` config to its `config/` folder with absolute paths, disable the plugin's web server,
enable `write-players-interval` and `write-markers-interval` in the plugin, and install
`systemd/bluemap-web.service`.

## Files

| File | What |
|---|---|
| `start.sh` | Starts Paper directly with the AOT cache, rebuilds the cache after updates |
| `lazymc.example.toml` | Freeze after 30 min, hold the player, whitelist only wake, start on boot |
| `setup/enable-sleep.sh` | Swap file, zswap with lz4, sysctls, freeze helper service |
| `setup/blink-freeze-helper.sh` | Moves a frozen server's memory to zswap and swap |
| `patches/lazymc-blink.patch` | lazymc #107 plus Blink's changes, on top of lazymc master |
| `systemd/` | Services for the server (lazymc in tmux), the freeze helper and BlueMap's web server |
| `tools/measure-startup.sh` | Time until "Done" |
| `tools/status-ping.py` | Server list ping, or a test join that wakes the server |
| `docs/` | The website |

## License

MIT, see [LICENSE](LICENSE). lazymc and the patch in `patches/` are GPL-3.0.
Not affiliated with Mojang, Microsoft or PaperMC.
