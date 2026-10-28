# Changelog

Blink is built on [lazymc](https://github.com/timvisee/lazymc) by Tim Visée; lazymc's own
changelog up to 0.2.11 is in its repository and in this repository's history.

## 2.3.0 (2026-10-28)

**First runs that work, on every OS and every kind of server.**

### Fixed
- **Windows installer**: it always stopped with "Checksum of blink.exe does not match" in Windows
  PowerShell 5.1 (the PowerShell that comes with Windows): the checksum file was read as bytes.
- **Windows**: a Java in `C:\Program Files` (where every Java installer puts it) broke the start
  command, because `JAVA=` was written without quotes.
- **A new server** (no `server.properties` yet) started on Blink's own port and failed. Blink now
  writes `server.properties` with the server's port first. On that first start of a new Paper
  server the PROXY protocol waits until Paper wrote its config, instead of failing every status
  check until the server was killed.
- **The PROXY protocol setting in Paper** follows `send_proxy_v2`, also when you turn it off, and
  `blink restore` turns it off again. Before, it stayed on and nobody could join without Blink.
- **A thawed server takes the join at once**, as 2.1.0 meant to: the state went to "starting" and
  the join still waited for a status check.
- **A join while a long frozen server shuts down** (`stop_after`) gets the "going to sleep"
  message, instead of being passed to a server that is stopping.
- **The sleeping MOTD** shows in the server list (the server's own MOTD was shown instead).
- **Saving before a freeze** uses `save-all flush`: every chunk is on disk before the freeze.
- **systemd service**: starts on kernels before 5.19 and without zswap (the `chown` of the cgroup
  files failed the unit), and restarts Blink if it stops by itself (`Restart=always`).
- **Memory reclaim** on kernels before 6.11, which don't know `swappiness=` in `memory.reclaim`.
- **`server.properties` is changed only once Blink has the public port**, so a Blink that can't
  start (your old server still running) leaves the server where it was.
- `before_freeze` works on Windows (`cmd /C`), the anonymous usage count too.
- `stop_timeout` defaulted to 300 instead of 150, `minimum_online_time` to 0 instead of 60.

### New
- **Fabric, Quilt, Forge, NeoForge and vanilla**: `blink init` and the installers recognise them.
  Forge and NeoForge start their java line directly (from `run.sh` / `run.bat`), so the freeze
  reaches Java. Vanilla's watchdog (`max-tick-time`) is turned off for frozen servers.
- **Freezing reaches everything the command starts** (the process group on Linux and macOS, the
  process tree on Windows): a start script works as `command`.
- **`blink restore`** and `--uninstall` for every installer: put back the server's port, Paper's
  watchdog and the PROXY protocol setting, and only those (your other edits stay).
- **Typing `stop`** in Blink's console stops the server and Blink, as Ctrl+C does.
- **Clear messages** for: the EULA not accepted, a server that quits while starting, the public
  port in use, a missing jar or Java, an empty or broken `command` (it used to crash at the first
  join). `blink config test` checks all of it, and `--verbose` exists.
- **Any Java works**; Java 26 adds the fast cold start (AOT cache). Older Javas start Paper without
  the cache, with a warning, instead of failing. The installer no longer stops when Java 26 is
  not in your package sources.
- **Running an installer again keeps your `blink.toml`** (it used to write a new one).
- The default heap reads the RAM on Windows and macOS too (it was always 4G there).
- `join.hold.timeout` is 25 s, below the client's own 30 s, so a slow start shows the message.
- CI runs the installers as users do (Windows PowerShell 5.1, `install.sh --portable`), a new
  server without `server.properties`, `stop` in the console, `blink restore` and the unit tests.

## 2.2.0 (2026-10-27)

- Two steps of sleep: compressed in RAM (zswap, lz4) for the first 10 minutes, then the swap file
  (`deep_sleep_after`): 7 MB of RAM, about 3 s to wake up from there.
- lz4 instead of zstd: joins after a freeze 0.5 to 0.9 s instead of 0.8 to 1.1 s.
- The Paper launcher turns off spark's background profiler.
- Website rebuilt in Minecraft's style, without external requests.

## 2.1.0 (2026-10-27)

- Linux, macOS and Windows; BungeeCord and Velocity networks, tested with a proxy and five servers.
- The Paper launcher is part of Blink; `blink init`; `install.sh --portable` and `install.ps1`.
- Built for 5 platforms and tested with a real Paper server on every OS for every change.

## 2.0.0 (2026-10-27)

- One program (1.6 MB, 1.5 MB of RAM) instead of a patched lazymc with scripts and a root helper.
- Saves before every freeze; stops a server frozen for a day (`stop_after`); real player IPs with
  the PROXY protocol; `wake_known_players`; `threads`; stop while frozen in 1 to 2 s.

## 1.1.0 and 1.0.0 (2026-10-27)

- Blink 1: lazymc with patches (Paper status fix from lazymc #107, fast polling while waking),
  Paper with Java's AOT cache, and a freeze with save first.
