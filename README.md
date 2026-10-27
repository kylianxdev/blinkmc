# paper-fast-wake

**A Minecraft server that sleeps when nobody plays and is back up in about 5 seconds when someone joins.**

Kylian de Groot, October 2026

| | Before | After |
|---|---|---|
| Paper start time ("Done") | 11.5 s | **4.4 s** (median of 25 real starts) |
| Wall time from launch to "Done" | 13.2 s | **5.0 s** |
| Join on a sleeping server until the player is on the real server | not possible | **5.1 s** |
| Free RAM on the VPS while nobody plays | 1.7 GB | **10.5 GB** |
| Server list (MOTD, icon) while asleep | | still works |
| Web map (BlueMap) while asleep | | still works |

---

## Abstract

A small private Minecraft server spends most of its life empty, yet it holds on to its 8 GB heap
and CPU time all day. Stopping it when nobody plays is easy; the hard part is starting it again so
fast that a player barely notices. We combine three things: **lazymc**, a proxy that keeps the
server list alive and starts the server on the first join; a set of **fixes to lazymc** so it
works with current Paper; and **Java's AOT cache** (Project Leyden) to cut Paper's own start from
11.5 s to about 4.4 s. Two details turned out to matter most: Paper has to be started *without*
its paperclip launcher so the cache can hold its classes, and Java 26's *streamable* AOT objects
are needed, because Minecraft's method type table is too big for the older cache format. Everything
here is a shell script, a config file and a 7 line patch; nothing in Paper or Minecraft is changed.

## 1. Intent

The server in this write-up is a survival world for a handful of friends on a VPS with 6 cores and
11.6 GB of RAM. Next to Minecraft it runs a live web map (BlueMap) and a stats website. Most of the
day nobody is online, but the server still reserved 8 GB and left about 1.7 GB for everything else.

The goals were:

1. **Off when nobody plays.** After 30 minutes without players, stop the server completely and give
   the memory back.
2. **Invisible to players.** The server stays in everyone's server list with its normal MOTD and
   icon. Joining a sleeping server just works; you wait a few seconds on "Joining world".
3. **Fast.** The wait should be short enough that it does not feel like a cold start. We set the
   target at under 5 seconds.
4. **Nothing else breaks.** The web map, backups and chat announcements keep working.
5. **No forks.** Stock Paper, stock Java from the Ubuntu archive, no mods.

## 2. Background

**lazymc** ([timvisee/lazymc](https://github.com/timvisee/lazymc)) listens on the public port in
front of the real server. While the server sleeps it answers server list pings itself. On a
login it starts the server and *holds* the client: the connection stays open until the server is
ready, then lazymc relays it, so the player joins without having to retry. When the server has been
empty for a while, lazymc stops it.

**Paper's start** consists of: starting the JVM, the paperclip launcher (which checks and unpacks
the real server jar), Minecraft's bootstrap (registering every block, item, entity, codec and data
fixer), loading data packs (recipes, advancements, loot tables, tags), loading the worlds and
enabling plugins. Most of this is class loading and code that runs exactly once, which is the worst
case for a JIT compiler and the best case for an ahead of time cache.

**The AOT cache** ([JEP 483](https://openjdk.org/jeps/483), [JEP 514](https://openjdk.org/jeps/514),
[JEP 515](https://openjdk.org/jeps/515), [JEP 516](https://openjdk.org/jeps/516)) lets you do a
training run in which the JVM records which classes the application loads and links and how its
methods behave. From that it builds a cache file. Later runs map that file and start with those
classes already loaded and linked, lambdas already spun up and method profiles already present.

## 3. Method

### 3.1 Measuring

All numbers come from `tools/measure-startup.sh`: it stops the server, starts it with a given
script in tmux and measures the wall time until Paper prints `Done (...)`. Paper's own number in
`Done (4.4s)` starts later (after the JVM and launcher are up), so both are reported. To see
*where* time went we used Java Flight Recorder during start (`-XX:StartFlightRecording`) and
`-Xlog:class+load` to see where every class came from.

Setup: Paper 26.3 build 200, OpenJDK 25.0.4 and 26.0.2 (Ubuntu packages), 6 vCPU (Intel Haswell),
11.6 GB RAM, an 838 MB world, plugins BlueMap 5.29 and spark.

### 3.2 Baseline

The original `start.sh` ran `java -jar paper.jar` with Aikar's flags, an 8 GB heap and
`-XX:+AlwaysPreTouch`. **13.2 s** until "Done" (Paper: 11.5 s).

### 3.3 Starting Paper without paperclip

`paper.jar` is a launcher. It unpacks the real server to `versions/` and its libraries to
`libraries/`, then loads them in **its own class loader**. The AOT cache only stores classes from
the normal class path, so with paperclip almost nothing of Minecraft can be cached.

The fix is to start the real server directly. `paper.jar` contains two lists that say exactly what
to put on the class path (`META-INF/versions.list` and `META-INF/libraries.list`), and the main
class is `org.bukkit.craftbukkit.Main`. `start.sh` reads those lists with a few lines of Python and
writes `classpath.txt`. After a Paper update it first runs
`java -Dpaperclip.patchonly=true -jar paper.jar`, which unpacks the new version and exits.

### 3.4 Dropping AlwaysPreTouch

`-XX:+AlwaysPreTouch` makes the JVM write to every page of the 8 GB heap before starting. That
helps a server that runs for weeks a little, and costs seconds on every start. For a server that
starts several times a day it is the wrong trade, so it is gone.

### 3.5 The AOT cache on Java 25: half of it works

A training run with `-XX:AOTMode=record` followed by `-XX:AOTMode=create` failed:

```
[error][aot,heap] Cannot archive the sub-graph referenced from [Ljava.util.concurrent.ConcurrentHashMap$Node; object size 262160, skipped.
[error][aot] An error has occurred while writing the shared archive file.
```

The object is the backing array of the JVM's `MethodType` intern table. Minecraft creates so many
method types (it is full of lambdas and codecs) that the array is a few bytes over the 256 KB
limit for one archived object. The cache file comes out empty.

With `-XX:-AOTClassLinking` the heap objects are left out and the cache builds. Start time went to
**7.8 s** (Paper: 7.5 s). Class load logging showed the limit of that version: 17,048 classes came
from the cache, but 14,427 did not. Almost all of those were lambda and hidden classes generated at
run time (865 from `Blocks` alone, 446 from `Items`), and those can only be cached *with* class
linking.

Two smaller traps on the way:

* `--enable-native-access=ALL-UNNAMED` (which paperclip's manifest sets) breaks building the cache
  with linked classes. It only silences a warning, so we left it out.
* The one step mode `-XX:AOTCacheOutput` builds the cache in a child JVM when the server exits. In
  tmux, that child was killed together with the session and the cache file stayed empty. Doing
  record and create as two explicit steps avoids it.

### 3.6 Java 26: streamable AOT objects

Java 26 has a second format for cached heap objects ([JEP 516](https://openjdk.org/jeps/516)):
instead of mapping them as one memory region, it loads them as a stream of objects, which works
with every garbage collector and has no 256 KB per object limit. It is not the default for G1 yet,
but a diagnostic switch turns it on:

```
-XX:+UnlockDiagnosticVMOptions -XX:+AOTStreamableObjects
```

With that, the full cache builds (230 MB, with class linking) and the start dropped to **4.9 s**
(Paper: 4.3 s). The final `start.sh` adds a cheap check to rebuild the cache after an update
(`stat` of `paper.jar` and `libjvm.so` instead of `java -version`, which itself costs half a second)
and measures **5.0 s** (Paper: 4.4 s).

Java 26 is not an LTS release. Paper 26.3 requires Java 25 or newer, so 26 is supported. The old
Java 25 stays installed as the system default; only `start.sh` points at 26.

### 3.7 lazymc on current Paper

lazymc 0.2.11 (March 2024) started the server fine but never noticed that it was up: the player
was held for 40 seconds and then kicked. With trace logging, lazymc's monitor kept asking for the
server's status and silently threw the answer away. Two known issues cause this: a status answer
with a server icon ([#102](https://github.com/timvisee/lazymc/issues/102)) and a MOTD written as a
chat component instead of a plain string, which is what Paper sends. Pull request
[#107](https://github.com/timvisee/lazymc/pull/107) fixes the decoding but is not merged yet. We
build lazymc from `master`, merge #107 and add our own small patch
(`patches/lazymc-fast-detect-clean-exit.patch`):

* **Status poll every 0.5 s instead of 2 s.** With a 4.4 s start, waiting up to 2 extra seconds to
  notice the server is up is a big part of the total. A status ping to localhost costs nothing.
* **Quit after one Ctrl+C.** lazymc stopped the server on the first Ctrl+C but needed a second one
  to quit, which made `systemctl stop` hang. Now it waits until the server has stopped and then
  exits.

### 3.8 Keeping the rest alive

* **Web map.** BlueMap's built in web server lives inside Paper and would sleep with it. We turned
  it off (`webserver.conf: enabled: false`) and run BlueMap's CLI as a separate web server
  (`java -jar bluemap-cli.jar -w`, 73 MB of memory), pointing at the same files. The plugin still
  renders and now writes live player positions and markers to the map storage
  (`write-players-interval: 3`, `write-markers-interval: 10` in `plugin.conf`) so the separate web
  server can serve them.
* **Scripts that type in the console** (backups, chat announcements) check first whether the real
  server is listening on its port, and skip or take the "server is off" path when it sleeps.
* **The daily restart** is no longer needed: every wake up is a fresh start.

## 4. Results

| Step | Wall time until "Done" | Paper's "Done" |
|---|---|---|
| Baseline: paperclip, Java 25, AlwaysPreTouch | 13.2 s | 11.5 s |
| Direct start, no AlwaysPreTouch, Java 25 AOT cache without class linking | 7.8 s | 7.5 s |
| Same, Java 26 with full AOT cache (streamable objects) | 4.9 s | 4.3 s |
| Final `start.sh` (with update check) | **5.0 s** | **4.4 s** |

In production, 25 cold starts over four days gave Paper times between 4.15 s and 5.13 s, median
4.35 s. A join on a sleeping server reached the real server (its encryption request) after 5.1 s.

While the server sleeps, the machine has 10.5 GB of free memory instead of 1.7 GB. lazymc itself
uses 6 MB.

Tick performance is not affected: the cache only changes how classes get loaded at start; the JIT
compiles hot code exactly as before.

## 5. What we learned the hard way

* **Bots wake servers.** For the first days, the server woke up about ten times a day, every time
  for a scanner or a bot with a random name, and then stayed up for 30 minutes. They could never
  get in (the whitelist is on), but waking up does not need a successful login. lazymc's
  `wake_whitelist = true` only lets players from `whitelist.json` wake the server. If your server
  has no whitelist, consider a list of known players anyway: lazymc reads `whitelist.json` even when
  Minecraft does not use it.
* **Profile before you tune.** The intuition was "the world loading is slow". The world took 0.5 s.
  The time was in one thread loading classes during Minecraft's bootstrap.
* **Read the error, not the summary.** "An error has occurred while writing the shared archive file"
  says nothing; the line above it named the exact object that did not fit.

## 6. Limitations

* **The first start after an update is slow.** It is the training run (about 13 s); the cache is
  built when it stops (about 5 s extra).
* **Plugin classes are not cached.** Plugins have their own class loaders. BlueMap and spark are
  loaded the normal way; with many plugins the gain will be smaller.
* **230 MB on disk** for the cache.
* **lazymc is a fork for now**, until #107 is merged and released.
* **Diagnostic flag.** `AOTStreamableObjects` is a diagnostic option in Java 26. If it changes in a
  later version, the cache build fails and `start.sh` starts without a cache (slow but working).
* **Hold timeout.** If the server ever takes longer than 40 s (the training run on a slow
  machine), the player is kicked with "try again in 20 seconds" instead of being held.

## 7. How to do this on your own server

You need a Linux machine with systemd and tmux, Paper 26 or newer, and Java 26 (`openjdk-26-jre-headless`
on Ubuntu 26.04).

**1. Fast start script.** Copy `start.sh` next to your `paper.jar`, set `HEAP` (and `JAVA` if your
Java 26 lives elsewhere) and make it executable. Start it once and stop it normally: that was the
training run. Every start after that uses the cache.

```bash
cp start.sh /opt/minecraft/ && chmod +x /opt/minecraft/start.sh
```

**2. Build lazymc with the fixes.**

```bash
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
git clone https://github.com/timvisee/lazymc && cd lazymc
git fetch origin pull/107/head:pr107 && git merge --no-edit pr107
git apply /path/to/paper-fast-wake/patches/lazymc-fast-detect-clean-exit.patch
cargo build --release && cp target/release/lazymc /opt/lazymc/
```

**3. Configure lazymc.** Copy `lazymc.example.toml` to your server folder as `lazymc.toml`. Set
`version` and `protocol` to what your server reports:

```bash
python3 tools/status-ping.py localhost
```

lazymc moves the real server to `127.0.0.1:25566` in `server.properties` by itself.

**4. Services.** Adjust the paths and user in `systemd/minecraft.service`, install it, and start it.
The console is still `tmux attach -t minecraft`.

**5. BlueMap (optional).** Download `bluemap-<version>-cli.jar`, copy your plugin's `maps/` and
`storages/` config into its `config/` folder, set absolute paths for the webroot and storage,
disable the plugin's web server, enable `write-players-interval` and `write-markers-interval` in
the plugin, and install `systemd/bluemap-web.service`.

**6. Test it.**

```bash
tools/measure-startup.sh ./start.sh          # while nobody is online
python3 tools/status-ping.py your.server     # what the server list shows while asleep
```

## Files

| File | What |
|---|---|
| `start.sh` | Starts Paper directly with the AOT cache, rebuilds the cache after updates |
| `lazymc.example.toml` | lazymc config: hold the player, sleep after 30 min, whitelist only wake |
| `patches/lazymc-fast-detect-clean-exit.patch` | 0.5 s status polling and a clean exit on one Ctrl+C |
| `systemd/minecraft.service` | lazymc in tmux, so the console keeps working |
| `systemd/bluemap-web.service` | BlueMap's web server, separate from the game server |
| `tools/measure-startup.sh` | Measures the time until "Done" |
| `tools/status-ping.py` | Server list ping, or a test join that wakes the server |

## License

MIT, see [LICENSE](LICENSE). lazymc is GPL-3.0; the patch in `patches/` falls under its license.
