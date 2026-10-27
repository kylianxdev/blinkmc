<p align="center">
  <img src="docs/assets/banner.png" alt="Blink: your Minecraft server sleeps when nobody plays and wakes up in 0.1 seconds" width="100%">
</p>

<p align="center">
  <a href="https://kylianxdev.github.io/blinkmc/"><b>Website</b></a> ·
  <a href="#7-install">Install</a> ·
  <a href="#4-results">Results</a> ·
  <a href="#blink-vs-lazymc">Blink vs lazymc</a>
</p>

<p align="center">
  <img alt="Paper 26+" src="https://img.shields.io/badge/Paper-26%2B-36c46b?style=flat-square">
  <img alt="Java 26" src="https://img.shields.io/badge/Java-26-f5c542?style=flat-square">
  <img alt="Wake up 0.1 s" src="https://img.shields.io/badge/wake%20up-0.1%20s-6c8cff?style=flat-square">
  <img alt="1.5 MB" src="https://img.shields.io/badge/blink-1.5%20MB%20RAM-36c46b?style=flat-square">
  <img alt="GPL-3.0" src="https://img.shields.io/badge/license-GPL--3.0-9aa5d6?style=flat-square">
</p>

# Blink

**The RAM solution for Minecraft servers and BungeeCord or Velocity networks.** Blink makes your
server sleep when nobody plays and wake up in 0.1 seconds when someone joins. The sleeping server gives most of its memory back to the machine, the server list keeps
showing your MOTD and icon, and players don't install anything. Stock Paper, stock Java, no mods.

Measured on our server (6 vCPU VPS, Paper 26.3, an 838 MB world). Your numbers depend on your
hardware; [`tools/`](tools) has the scripts to measure them yourself.

| | Stock Paper | With Blink |
|---|---|---|
| Join on a sleeping server until the real server takes over | not possible | **0.1 to 0.4 s** |
| Switch (BungeeCord or Velocity) to a sleeping server until in game | not possible | **0.3 to 1.7 s** |
| Cold start (after a reboot or an update) | 13.2 s | **4.4 s** |
| Memory of a server nobody plays on | 3.0 GB in RAM, 8 GB reserved | **7 MB** after 10 minutes (0.3 GB before) |
| Blink itself | | **1.5 MB** of RAM, one 1.6 MB file |
| Server list (MOTD, icon) while asleep | | works |

Blink is one small program in front of your server. It answers the server list while the server
sleeps, saves and freezes the server 30 seconds after the last player left, moves its memory out of
RAM, and wakes it up as soon as someone joins. It is built on
[lazymc](https://github.com/timvisee/lazymc) by Tim Visée.

**If Blink saves you RAM (or your friends' patience), a star helps other server owners find it.** ⭐

## Who it is for

* **A server for you and your friends.** It's empty most of the day. While it sleeps it costs a few
  hundred MB instead of gigabytes, so a smaller VPS does, or one machine runs more servers.
* **Hosting on your own PC.** A frozen server uses no CPU at all and hands its memory back, so your
  games and browser get it, and the machine can idle instead of ticking an empty world all day.
* **Several servers on one machine.** Survival, creative, a modded test world, an event map: only
  the ones people are on stay awake.
* **Networks behind BungeeCord or Velocity.** A Blink in front of each backend server: lobby,
  survival, minigames, a personal world per player. Only the servers people are on stay awake, and
  switching to a sleeping one with `/server` just works. [Tested](#networks-bungeecord-and-velocity)
  with one proxy and five servers.

## Install

Stop your server and go to the folder with your `paper.jar`.

**Linux, full install** (a service, swap and zswap so a frozen server leaves RAM; needs root):

```bash
curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | sudo bash
```

**Linux or macOS, portable** (no root, changes nothing on your system; just `blink` in the folder):

```bash
curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | bash -s -- --portable
./blink start
```

**Windows** (PowerShell, no administrator rights needed):

```powershell
irm https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.ps1 | iex
```

Then start the server with `start-blink.bat`. More options in [Install](#7-install).

| | Freeze and wake | Memory of a frozen server |
|---|---|---|
| Linux | yes (SIGSTOP) | moved to zswap and swap by Blink (full install) |
| macOS | yes (SIGSTOP) | compressed by macOS itself when it needs RAM |
| Windows | yes (NtSuspendProcess) | handed to Windows' memory compression by Blink |

All three are tested on every change with a real Paper server (GitHub Actions).

## Networks: BungeeCord and Velocity

Put a Blink in front of every backend server, and point the proxy at Blink's ports. A player who
switches to a sleeping server with `/server` (or a plugin that sends them there) wakes it up; the
proxy just sees a server that answers a little later.

**Tested** with one proxy, a lobby and four "personal worlds", each a Paper server behind its own
Blink, with bots that log in for real and switch servers (`tools/network-test.py`), on BungeeCord
and on Velocity (legacy and modern forwarding):

| | BungeeCord | Velocity (modern) |
|---|---|---|
| Join the network while everything sleeps (lobby thaws) | 0.8 to 1.4 s | 1.1 to 1.6 s |
| Switch to your own sleeping world | 0.3 to 1.5 s | 1.2 to 1.7 s |
| Four players at once, each to their own sleeping world | 0.24 to 0.31 s each | 1.3 to 1.7 s each |
| The first visit after a server (re)started | about 3 to 5 s | about 3 s |
| Switch between awake worlds | 0.1 to 0.3 s | 0.3 to 0.6 s |
| Everyone left: all five servers saved and frozen | after 30 s | after 30 s |
| RAM of the five servers, frozen (first 10 minutes) / awake with players | 1.0 GB / 2.9 GB | 1.0 GB / 2.6 GB |

(After 10 minutes asleep each server drops to about 7 MB, deep sleep, which this test didn't run long enough to reach; a switch to a server in deep sleep takes about 3 s. The first visit after a restart is slower because Java hasn't compiled the login code yet; after
that every thaw is fast. Each Blink uses 1.4 MB; the proxy itself stays awake.)

Setup, per backend server:

1. In each server folder: `blink init` (or the installer with `--name` per server). Blink takes the
   port from `server.properties` as its public port and moves the server 10000 ports up.
2. In the proxy's server list, use those public ports, as you would without Blink.
3. Forwarding as usual: BungeeCord `ip_forward: true` with `settings.bungeecord: true` in each
   `spigot.yml`, or Velocity modern forwarding with `proxies.velocity` in each `paper-global.yml`.
4. Set `send_proxy_v2 = false` in each `blink.toml`: behind a proxy the players' IPs come with the
   forwarding.
5. With `stop_after` (deep sleep), the first join after a day is a cold start of about 4 s; raise
   the proxy's timeout to be safe (BungeeCord `server_connect_timeout: 15000`, Velocity
   `connection-timeout = 15000`).

**Safe for your world?** Blink saves the world before every freeze and lets Paper save and stop
normally when you stop it, also while frozen. Nothing in Paper or your world is changed. Still: try
it on a copy or with a fresh backup first, like any tool that runs your server. `--uninstall` puts
everything back.

## Blink vs lazymc

Same machine, same Paper 26.3 server, measured October 2026.

| | lazymc 0.2.11 | Blink 2 |
|---|---|---|
| Works with Paper 26.3 | **no**: never sees that the server is up; every player is held 25 s, then kicked | yes |
| Join on a sleeping server | never gets in (if it did: a 13 s cold start) | **0.1 to 0.4 s** (thaw), 4.4 s cold |
| Memory of the proxy | 5.7 MB, 10 threads | **1.5 MB**, 2 threads |
| Program | 3.9 MB + glibc | **1.6 MB**, one static file |
| Sleeping server | stopped (0 MB, cold start) or frozen with all its memory (1 to 3 GB) | **7 MB** after 10 minutes frozen |
| Saves before sleeping | no | yes, `save-all` and waits for it |
| Stop while asleep | Ctrl+C never reaches a frozen server; systemd kills it after its timeout | thaws, saves and stops in 1 to 2 s |
| Players' real IPs | manual setup on both sides | automatic (PROXY protocol) |
| Console | needs the terminal | through Blink, also for scripts |
| Fast start for Paper | no | built in (Java AOT cache) |
| Throughput | | about 1 GB/s on one thread, more with `threads` |

The rest of this README is written as a short paper: what we wanted, how we got there and what
failed on the way.

Kylian de Groot, October 2026

---

## Abstract

A small private Minecraft server is empty most of the day, yet it holds on to gigabytes of memory.
Stopping it when nobody plays is easy; starting it again so fast that a player does not notice is
the hard part. Blink is a sleep proxy (built on **lazymc**) with two ways back:

1. **Freeze.** The JVM is paused instead of stopped. Linux moves its memory into compressed RAM
   (zswap) and swap, and the heap shrinks itself first when the server is quiet. A sleeping server
   keeps about 0.3 GB of its 3 GB. A join continues the paused process: **0.1 to 0.4 s**.
2. **Cold start.** For reboots and updates. Paper is started without its paperclip launcher so
   **Java's AOT cache** (Project Leyden) can hold all of its classes. Java 26's streamable AOT
   objects are needed, because Minecraft's method type table is too big for the older format.
   **13.2 s down to 5.0 s.**

All of it is one 1.6 MB program that uses 1.5 MB of RAM. Nothing in Paper or Minecraft is changed.

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

Blink's Paper launcher (`src/launcher.rs`) reads `META-INF/versions.list` and `META-INF/libraries.list` from `paper.jar`, writes
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
the launcher (which uses `stat` instead of `java -version`, itself half a second). Over 25 real cold
starts Paper reported a median of 4.35 s.

### 3.6 Under a second: freeze instead of stop

A cold start under one second is not realistic: the JVM and Minecraft's registries alone take longer.
So the fast path does not start at all. lazymc can freeze the server (`SIGSTOP`) and continue it
(`SIGCONT`). On its own that wakes instantly but keeps all memory, which defeats the point. Blink
adds:

* **Memory out of the way, in two steps.** A few seconds after a freeze, Blink writes to its
  service's cgroup file `memory.reclaim` (the service lets it, with one `chown` before it starts).
  The kernel compresses the heap into **zswap** with lz4: about 0.3 GB stays in RAM and a join is
  back in the world in **0.5 to 0.9 s**. After 10 minutes frozen (`deep_sleep_after`), Blink closes
  zswap for its cgroup and reclaims a few more rounds: everything goes to the swap file and **7 MB**
  stays in RAM (measured on the live server: 522 MB to 7 MB in 5 seconds). Waking from there takes
  about 3 s, because the server reads its memory back from disk.
* **lz4, not zstd.** zstd compresses better (3.9:1 against 2.7:1), but unpacking is slower: a join
  took 0.8 to 1.1 s with zstd and 0.5 to 0.9 s with lz4. Since a long sleep goes to disk anyway, the
  faster one wins.
* **What we tried and dropped.** Reading the memory back ahead of time (on a server list ping, or
  in parallel threads on a thaw) pulled back 1.2 GB the server didn't need and made joins slower.
  Keeping 300 to 500 MB "hot" in RAM didn't make a join faster: half of the wake time is the thaw
  itself (0.3 to 0.5 s without moving any memory at all).
* **A small heap, and no periodic GC.** The heap starts at 256 MB instead of the maximum
  (`-Xms256M -Xmx8G`) and gives memory back after collections, so a quiet server is small when it
  freezes. We also tried a GC every 15 seconds of quiet
  (`G1PeriodicGCInterval`): it made a frozen server only a few MB smaller, but right after a thaw
  it fired at once and pulled the whole heap back into RAM, so a join took 3 to 5 seconds instead
  of under one. It's gone.
* **Let the player through right after a thaw.** A thawed server was running when it was frozen,
  so Blink passes the join on at once instead of first asking the server for its status (a just
  thawed server can take seconds to answer that).
* **Save, then freeze after 30 seconds.** With a 0.3 s wake up there is no reason to wait long.
  Minecraft does not save when it pauses an empty server, so Blink types `save-all` in the console,
  waits for "Saved the game", checks that nobody joined meanwhile, and only then freezes. A reboot of the machine while the server is frozen loses nothing.
* **No read ahead from swap** (`vm.page-cluster=0`). With zswap, reading 8 pages per fault only adds
  latency. Together with the next point this took a wake up from 1.2 s to 0.1 s.
* **Blink polls every 50 ms while waking**, instead of lazymc's 2 s, and does not poll a frozen server
  at all (it cannot answer; each poll would only wait for a timeout).
* **Paper's watchdog** sees a gap of minutes or days after a thaw and kills the server as hung.
  Blink's Paper launcher sets `timeout-time` in `spigot.yml` and `early-warning-delay` in
  `paper-global.yml` to 20 days, and `restart-on-crash` off (it started a second server outside
  the proxy).
* **Stopping a frozen server.** Paper runs in its own process group with its console piped through
  Blink, so Ctrl+C and `systemctl stop` reach Blink, which thaws the server, lets Paper save and
  stop, and then quits.
* **Deep sleep.** After a day frozen (`stop_after`), Blink stops the server completely: then only
  Blink's 1.5 MB is left, and the next join is a 4.4 s cold start.
* **`wake_on_start`.** After a reboot the server starts right away and freezes after the idle time,
  so even the first join of the day is a thaw and not a cold start.

Measured: three wake ups in a row of **0.1 s, 0.4 s and 0.4 s**, the same process every time.

### 3.7 From lazymc to Blink

lazymc 0.2.11 started the server but never noticed it was up: players were held for 40 seconds and
then kicked. Its monitor silently threw away Paper's status answer. Two known issues cause this: a
server icon in the answer ([#102](https://github.com/timvisee/lazymc/issues/102)) and a MOTD written
as a chat component. Pull request [#107](https://github.com/timvisee/lazymc/pull/107) fixes the
decoding but is not merged yet.

Blink started as a patched lazymc next to a few shell scripts and a root helper. Blink 2 is one
program: lazymc `master` with #107, the changes above, a built in Paper launcher, the console, the
save and the memory reclaim. And it got small:

| | lazymc 0.2.11 | Blink 2 |
|---|---|---|
| Memory (RSS) | 5.7 MB | 1.5 MB |
| Of which private | 472 kB | 148 kB |
| Threads | 10 | 2 |
| File | 3.9 MB + glibc | 1.6 MB, static |

How: a static musl build optimized for size, one tokio thread (more with `advanced.threads`), a
logger of a few lines instead of `pretty_env_logger` (which pulls in a regex engine), polling a few
files every 3 s instead of the `notify` watcher thread, and no lobby or RCON code. One thread still
passes about **1 GB/s** of player traffic (100 connections, `tools/` has the test); four pass 2.1 GB/s.

Players' real IP addresses reach the server with the PROXY protocol; the launcher turns it on in
Paper. Without it every player shows up as 127.0.0.1 and IP bans don't work.

### 3.8 Keeping the rest alive

* **Web map.** BlueMap's built in web server lives inside Paper and would freeze with it. We run
  BlueMap's CLI as a separate web server (`java -jar bluemap-cli.jar -w`, 73 MB) on the same files,
  and let the plugin write live player positions and markers to its storage.
* **Scripts that type in the console** (backups, chat announcements) still work: the console goes
  through Blink. They check first whether the server is really awake. A frozen server still listens on its port, but would only run the commands after
  it wakes up.
* **No daily restart needed.** After a day frozen Blink stops the server, so the next session is a
  fresh start anyway.

## 4. Results

| Path | Time | Notes |
|---|---|---|
| Stock Paper (paperclip, Java 25, AlwaysPreTouch) | 13.2 s | Paper: 11.5 s |
| Direct start, Java 25 AOT cache without class linking | 7.8 s | Paper: 7.5 s |
| Direct start, Java 26 full AOT cache | 4.4 to 5.0 s | Paper: median 4.35 s over 25 starts |
| **Thaw a frozen server (join until the real server answers)** | **0.1 to 0.4 s** | memory in zswap and swap |

| Memory | Before Blink | Awake, empty | Frozen |
|---|---|---|---|
| Minecraft service in RAM | 8 GB (AlwaysPreTouch) | 1.2 GB | 0.3 GB, then **7 MB** after 10 minutes |
| Blink | | 1.5 MB | 1.5 MB |
| BlueMap web server (optional) | | 73 MB | 73 MB |

Tick performance is not affected. The AOT cache only changes how classes get loaded; hot code is JIT
compiled as before. A frozen server uses no CPU at all.

### Benchmark with simulated players

On a copy of the same world (no plugins, 6 GB heap), players were simulated with `/forceload`: each
"player" loads the 25 x 25 chunks of view distance 12, around one of the four most built up
regions. `tools/benchmark.sh` does the full run.

| | Value |
|---|---|
| Cold start (3 runs) | 4.4, 4.5, 4.8 s (Paper: 3.9 to 4.2 s) |
| Awake, 0 players | 1.8 GB |
| Awake, 1 / 2 / 4 players | 2.1 / 2.6 / 2.8 GB |
| Frozen right after a 4 player session (saved first) | 0.6 GB in RAM, 2.7 GB moved out |
| Frozen, after 10 minutes (live server) | 7 MB in RAM |
| Wake up after the 4 player session | 0.3 s |

## 5. What we learned the hard way

* **Bots wake servers.** In the first days the server woke up about ten times a day, every time for
  a scanner or a bot, and then stayed up for 30 minutes. Waking needs no successful login. With a
  freeze after 30 seconds that costs almost nothing, so Blink lets anyone wake the server (with
  the whitelist on: only whitelisted players). `wake_known_players` limits it to players who
  joined before, if you want.
* **Profile before you tune.** The guess was "the world loads slowly". The world took 0.5 s; the time
  was one thread loading classes.
* **Read the line above the error.** "An error has occurred while writing the shared archive file"
  says nothing. The line before it named the exact object that did not fit.
* **A paused process has a clock problem.** Anything that measures time (watchdogs, timeouts) sees a
  frozen server as a hung server.

## 6. Limitations

* **The first start after an update is slow** (the training run, about 13 s), and the start after it
  builds the cache (a few seconds extra). With `wake_on_start` that mostly happens without anyone
  waiting for it.
* **Plugin classes are not cached** (they have their own class loaders). With many plugins the cold
  start gain is smaller. Thawing is not affected.
* **No watchdog for real hangs** while the timeouts are raised.
* **Disk**: 230 MB for the cache, plus a swap file.
* **Diagnostic flag.** If `AOTStreamableObjects` changes in a later Java, the cache build fails and
  the launcher simply starts without a cache.
* **One server per Blink.** Behind BungeeCord or Velocity: one Blink per backend server (each uses
  about 1.4 MB). The proxy itself stays awake.
* **Moving a frozen server's memory out of RAM** is strongest on Linux (zswap and swap, set up by the
  full install). On macOS and Windows the OS compresses it itself; it stays a bit bigger.

## 7. Install

The commands are at the top. The full install needs Linux with systemd (Ubuntu, Debian and most
others), on x86_64 or ARM, and a Paper server (or a Paper fork such as Purpur). Stop your server, go
to its folder and run:

```bash
curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | sudo bash
```

Options go after `bash -s --`:

```bash
curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | sudo bash -s -- --heap 6G --sleep 120
```

| Option | Default | |
|---|---|---|
| `--dir PATH` | current folder | server folder |
| `--heap SIZE` | 60% of RAM, at most 8G | maximum Java heap |
| `--sleep SECONDS` | 30 | save and freeze this many seconds after the last player left |
| `--threads N` | 1 | threads for the players' traffic; 2 to 4 for hundreds of players |
| `--name NAME` | minecraft | systemd service name (use another one per server) |
| `--swap SIZE` | heap + 2G | swap file, only when there is no swap yet |
| `--portable` | | no root, no service, no system changes (always on macOS) |
| `--uninstall` | | undo everything for this server |

What it does, step by step:

1. Finds your Paper jar and who owns the files (the server runs as that user).
2. Installs Java 26, tmux and python3 when they are missing (apt).
3. Downloads `blink` (checksum checked) to `/opt/blinkmc`.
4. Writes `blink.toml` in your server folder. The protocol version and your MOTD are read from your
   server. `server.properties` is kept as `server.properties.before-blink`.
5. Sets up a swap file (if you have none), zswap with lz4, and two sysctls.
6. Installs and starts the `minecraft` service.

Running it again updates Blink (and Blink 1 installs are cleaned up).

Afterwards:

```bash
sudo -u <user> tmux -L minecraft attach -t minecraft   # console (leave with Ctrl+B, then D)
sudo systemctl stop minecraft                          # saves and stops, also when frozen
python3 tools/status-ping.py localhost --join --name YourName   # test a wake up
```

**Whitelist.** Optional. With the whitelist on in `server.properties`, only whitelisted players can
wake the server. With it off, anyone can, also new players. Bots that wake it only keep it awake
until the next freeze; `wake_known_players = true` in `blink.toml` stops them, but also new players.

**All settings** are in [`res/blink.toml`](res/blink.toml), with an explanation for each.

**Anonymous usage count.** Once a day Blink downloads a 1 byte file from this project's GitHub
releases, and GitHub counts the downloads. That is the only way we know how many servers use
Blink. Nothing is sent: no server name, no players, no settings. Turn it off with
`usage_stats = false` in `blink.toml` or `DO_NOT_TRACK=1`; Blink says at start whether it is on.

**Without the installer.** Put the `blink` binary for your OS from the
[releases](https://github.com/kylianxdev/blinkmc/releases/latest) in your server folder, run
`blink init` (writes `blink.toml`) and `blink start`.

**BlueMap (optional).** BlueMap's web server would freeze with the server. Run BlueMap's CLI as its
own web server instead (`systemd/bluemap-web.service`, see section 3.8).

**Build it yourself.** The installer uses the binary from the
[latest release](https://github.com/kylianxdev/blinkmc/releases/latest). To build it:

```bash
cargo build --release                      # any OS; the release build for Linux uses musl:
cargo build --release --target x86_64-unknown-linux-musl
sudo BLINK_BINARY=target/x86_64-unknown-linux-musl/release/blink bash install.sh
```

## Files

| File | What |
|---|---|
| `src/` | Blink (Rust, based on lazymc); `src/blink.rs` has the console, save and memory reclaim |
| `src/launcher.rs` | The built in Paper launcher (AOT cache, watchdog, PROXY protocol), every OS |
| `src/action/init.rs` | `blink init`: writes `blink.toml` for the server in a folder |
| `res/blink.toml` | All settings, explained |
| `install.sh`, `install.ps1` | Installers for Linux and macOS, and for Windows |
| `.github/workflows/build.yml` | Builds for 5 platforms and tests a real server on Linux, macOS and Windows |
| `systemd/bluemap-web.service` | Optional: BlueMap's web server, separate from the game server |
| `tools/smoke-test.py` | End to end test: start, freeze, wake up, stop (what CI runs) |
| `tools/network-test.py` | A proxy with five servers: bots join, switch worlds, leave; times and RAM |
| `tools/netbot.py` | A tiny Minecraft 26.3 client for those tests (login, configuration, `/server`) |
| `tools/thaw-bench.py` | Repeatedly: switch to a frozen server through the proxy, time until in game |
| `tools/benchmark.sh` | Cold starts, memory with simulated players, freeze and wake |
| `tools/proxy-bench/` | Throughput of the proxy |
| `tools/status-ping.py` | Server list ping, or a test join that wakes the server |
| `docs/` | The website |

## License

GPL-3.0, see [LICENSE](LICENSE). Blink is built on [lazymc](https://github.com/timvisee/lazymc) by
Tim Visée (GPL-3.0); its history is part of this repository. Not affiliated with Mojang, Microsoft or
PaperMC.
