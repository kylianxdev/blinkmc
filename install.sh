#!/usr/bin/env bash
# Blink installer: your Minecraft server sleeps when nobody plays and wakes up in a blink.
#
#   cd /path/to/your/server        # the folder with paper.jar
#   curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | sudo bash
#
# Without root (and on macOS): only puts blink in your server folder, changes nothing else
#   curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | bash -s -- --portable
#
# Options (after "bash -s --" when piping, e.g. "| sudo bash -s -- --heap 6G"):
#   --portable       no root, no service, no system changes: blink and blink.toml in the server folder
#   --dir PATH       server folder (default: the current folder)
#   --heap SIZE      maximum Java heap, e.g. 6G (default: 60% of the RAM, at most 8G)
#   --sleep SECONDS  save and freeze this many seconds after the last player left (default: 30)
#   --threads N      threads for the players' traffic (default: 1, plenty below ~1000 players)
#   --name NAME      name of the systemd service (default: minecraft)
#   --swap SIZE      size of the swap file when there is none yet (default: heap + 2G)
#   --uninstall      undo everything for this server (keeps swap and zswap)
#   BLINK_BINARY=/path/to/blink   use a blink you built yourself instead of the release
#
# The full install (Linux, root) also installs Java 26 if needed, sets up a swap file and zswap
# (so a frozen server's memory can leave RAM) and runs everything as a systemd service. Your own
# files stay as they are (server.properties is kept as server.properties.before-blink, because
# Blink moves the server to another port in there).
set -euo pipefail

REPO=${BLINK_REPO:-kylianxdev/blinkmc}
REF=${BLINK_REF:-main}
RAW="https://raw.githubusercontent.com/$REPO/$REF"
OPT=/opt/blinkmc

DIR=$PWD HEAP="" SWAP="" NAME=minecraft SLEEP=30 THREADS=1 UNINSTALL=0 PORTABLE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dir) DIR=$2; shift ;;
    --heap) HEAP=$2; shift ;;
    --swap) SWAP=$2; shift ;;
    --name) NAME=$2; shift ;;
    --sleep) SLEEP=$2; shift ;;
    --threads) THREADS=$2; shift ;;
    --portable) PORTABLE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    -h|--help) sed -n '2,25p' "$0" 2>/dev/null || echo "see $RAW/install.sh"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done

if [ -t 1 ]; then G=$'\e[32m' Y=$'\e[33m' R=$'\e[31m' B=$'\e[1m' N=$'\e[0m'; else G="" Y="" R="" B="" N=""; fi
step() { echo "${G}==>${N} ${B}$*${N}"; }
warn() { echo "${Y}  ! $*${N}"; }
die() { echo "${R}  x $*${N}" >&2; exit 1; }

DIR=$(cd "$DIR" && pwd)
cd "$DIR"

# which blink: by OS and CPU
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ASSET=blink-linux-x64 ;;
  Linux-aarch64|Linux-arm64) ASSET=blink-linux-arm64 ;;
  Darwin-arm64) ASSET=blink-macos-arm64; PORTABLE=1 ;;
  Darwin-x86_64) ASSET=blink-macos-x64; PORTABLE=1 ;;
  *) die "no prebuilt blink for $(uname -s) $(uname -m); build it from source (see the README)" ;;
esac

download_blink() {   # $1: where to put it
  if [ -n "${BLINK_BINARY:-}" ]; then
    install -m 755 "$BLINK_BINARY" "$1"
    return
  fi
  local tmp; tmp=$(mktemp -d)
  curl -fsSL "https://github.com/$REPO/releases/latest/download/$ASSET" -o "$tmp/$ASSET"
  curl -fsSL "https://github.com/$REPO/releases/latest/download/$ASSET.sha256" -o "$tmp/$ASSET.sha256"
  if command -v sha256sum >/dev/null; then (cd "$tmp" && sha256sum -c --quiet "$ASSET.sha256"); else (cd "$tmp" && shasum -a 256 -c --quiet "$ASSET.sha256"); fi \
    || die "checksum of blink does not match"
  install -m 755 "$tmp/$ASSET" "$1"
  rm -rf "$tmp"
}

# ---------------------------------------------------------------- portable: no root, no changes
if [ "$PORTABLE" = 1 ] && [ "$UNINSTALL" = 0 ]; then
  step "Putting blink in $DIR"
  download_blink "$DIR/blink"
  echo "  $("$DIR/blink" --version)"
  step "Writing blink.toml"
  args=(--sleep "$SLEEP" --threads "$THREADS" --force)
  [ -n "$HEAP" ] && args+=(--heap "$HEAP")
  "$DIR/blink" init "${args[@]}"
  cat <<D

${G}${B}Done.${N} Start your server with Blink from now on:

  cd $DIR && ./blink start

  It needs Java 26 (the newest JDK on your machine is used; set JAVA= in blink.toml to choose).
  Keep it running in tmux or screen if you close the terminal. Stop it with Ctrl+C: the server
  saves and stops, also while it is frozen.

D
  exit 0
fi

# ---------------------------------------------------------------- full install (Linux, root)
[ "$(id -u)" = 0 ] || die "run it with sudo (or use --portable to install without root)"
command -v systemctl >/dev/null || die "the full install needs systemd; use --portable"
UNIT=/etc/systemd/system/$NAME.service

# Blink 1 left a lazymc.toml, a start.sh and a separate freeze helper; Blink 2 does it all itself
cleanup_blink1() {
  systemctl disable --now "blink-freeze-helper@$NAME" 2>/dev/null || true
  rm -f /etc/systemd/system/blink-freeze-helper@.service
  grep -qs "^# Blink:" "$DIR/lazymc.toml" && rm -f "$DIR/lazymc.toml"
  if grep -qs "paper.aot" "$DIR/start.sh"; then
    if [ -f "$DIR/start.sh.before-blink" ]; then mv -f "$DIR/start.sh.before-blink" "$DIR/start.sh"; else rm -f "$DIR/start.sh"; fi
  fi
  rm -f "$DIR/lazymc.toml.before-blink"
}

if [ "$UNINSTALL" = 1 ]; then
  step "Removing Blink from $DIR (service $NAME)"
  systemctl disable --now "$NAME" 2>/dev/null || true
  rm -f "$UNIT"
  cleanup_blink1
  [ -f "$DIR/server.properties.before-blink" ] && mv -f "$DIR/server.properties.before-blink" "$DIR/server.properties" && echo "  restored server.properties"
  rm -f "$DIR"/blink "$DIR"/blink.toml "$DIR"/paper.aot "$DIR"/paper.aot.config "$DIR"/paper.aot.stamp "$DIR"/classpath.txt
  systemctl daemon-reload
  echo "  done. Swap, zswap and /opt/blinkmc are left in place (other servers may use them)."
  echo "  Paper's watchdog settings in spigot.yml stay raised; set timeout-time back to 60 if you like."
  exit 0
fi

step "Checking $DIR"
JAR=$(ls paper.jar *.jar 2>/dev/null | head -1 || true)
[ -n "$JAR" ] || die "no server jar here (Paper, Purpur and other Paper forks work)"
OWNER=$(stat -c %U "$JAR")
[ "$OWNER" != root ] || warn "the server files belong to root; the server will run as root"
echo "  runs as: $OWNER"

# the public port: from before Blink, else from an earlier Blink install, else server.properties
PORT=$(sed -n 's/^server-port=//p' server.properties.before-blink 2>/dev/null | tail -1 || true)
[ -n "$PORT" ] || PORT=$(sed -n 's/^address = "0.0.0.0:\([0-9]*\)"/\1/p' blink.toml lazymc.toml 2>/dev/null | head -1 || true)
[ -n "$PORT" ] || PORT=$(sed -n 's/^server-port=//p' server.properties 2>/dev/null | tail -1 || true)
PORT=${PORT:-25565}
if systemctl is-active -q "$NAME" 2>/dev/null; then
  step "Stopping the running $NAME service"
  systemctl stop "$NAME"
fi
if ss -ltnH "sport = :$PORT" | grep -q .; then
  die "port $PORT is in use. Stop your Minecraft server first, then run this again."
fi
grep -qs '^eula=true' eula.txt || warn "eula.txt does not say eula=true yet; the server won't start until you accept the Minecraft EULA there"

step "Installing what is missing"
JAVA=""
for j in /usr/lib/jvm/*/bin/java; do
  [ -x "$j" ] || continue
  v=$("$j" -XshowSettings:properties -version 2>&1 | sed -n 's/.*java.specification.version = //p')
  [ "${v%%.*}" -ge 26 ] 2>/dev/null && JAVA=$j
done
need=()
command -v tmux >/dev/null || need+=(tmux)
command -v curl >/dev/null || need+=(curl)
[ -n "$JAVA" ] || need+=(openjdk-26-jre-headless)
if [ ${#need[@]} -gt 0 ]; then
  command -v apt-get >/dev/null || die "please install: ${need[*]} (Java 26 or newer)"
  apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${need[@]}" >/dev/null
  [ -n "$JAVA" ] || JAVA=$(ls /usr/lib/jvm/*26*/bin/java | head -1)
fi
echo "  java: $JAVA"

step "Downloading blink to $OPT"
mkdir -p "$OPT"
download_blink "$OPT/blink"
echo "  $("$OPT/blink" --version)"

step "Setting up the server folder"
cleanup_blink1
args=(--sleep "$SLEEP" --threads "$THREADS" --force)
[ -n "$HEAP" ] && args+=(--heap "$HEAP")
# the port Blink should listen on, also when server.properties was already rewritten
[ -f server.properties.before-blink ] || { [ -f server.properties ] && cp -p server.properties server.properties.before-blink; }
sed -i "s/^server-port=.*/server-port=$PORT/" server.properties.before-blink 2>/dev/null || true
sudo -u "$OWNER" env JAVA_HOME="$(dirname "$(dirname "$JAVA")")" "$OPT/blink" init "${args[@]}"
"$OPT/blink" config test -c blink.toml >/dev/null 2>&1 || die "blink.toml has an error"

step "Setting up swap and zswap"
if ! swapon --show=NAME --noheadings | grep -q .; then
  h=$(sed -n 's/.*HEAP=\([0-9]*\)G.*/\1/p' blink.toml | head -1); h=${h:-4}
  [ -n "$SWAP" ] || SWAP=$(( h + 2 ))G
  avail=$(df --output=avail -BG / | tail -1 | tr -dc 0-9)
  [ "${SWAP%G}" -lt $(( avail / 2 )) ] || die "not enough disk space for a ${SWAP} swap file (use --swap)"
  fallocate -l "$SWAP" /swapfile && chmod 600 /swapfile && mkswap -q /swapfile && swapon /swapfile
  grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
  echo "  swap file: $SWAP"
else
  echo "  swap: already there ($(swapon --show=SIZE --noheadings | head -1 | tr -d ' '))"
fi
cat > /etc/sysctl.d/90-blink.conf <<'S'
# Blink: keep a running server in RAM, and no read ahead from swap (pointless with zswap)
vm.swappiness=10
vm.page-cluster=0
S
sysctl -q -p /etc/sysctl.d/90-blink.conf
modprobe zstd 2>/dev/null || true
echo zstd > /etc/modules-load.d/blink-zswap.conf
cat > /etc/tmpfiles.d/blink-zswap.conf <<'T'
w /sys/module/zswap/parameters/compressor - - - - zstd
w /sys/module/zswap/parameters/max_pool_percent - - - - 25
w /sys/module/zswap/parameters/enabled - - - - Y
T
systemd-tmpfiles --create /etc/tmpfiles.d/blink-zswap.conf 2>/dev/null || true
[ "$(cat /sys/module/zswap/parameters/enabled 2>/dev/null)" = Y ] && echo "  zswap: on ($(cat /sys/module/zswap/parameters/compressor))" \
  || warn "zswap is not available in this kernel; frozen servers go to the swap file only (a bit slower to wake)"

step "Installing the service"
cat > "$UNIT" <<U
[Unit]
Description=Minecraft server $NAME (Blink: freezes when nobody plays, wakes up on join)
After=network-online.target
Wants=network-online.target

[Service]
User=$OWNER
WorkingDirectory=$DIR
# Blink runs as $OWNER and moves a frozen server's memory out of RAM itself (memory.reclaim)
ExecStartPre=+/bin/sh -c 'chown $OWNER /sys/fs/cgroup/system.slice/%n/memory.reclaim'
# in tmux, so the console stays reachable: sudo -u $OWNER tmux -L $NAME attach -t $NAME
ExecStart=/usr/bin/tmux -L $NAME new-session -d -s $NAME $OPT/blink start -c $DIR/blink.toml
# SIGINT to blink: it thaws a frozen server, lets it save and stop, then quits
ExecStop=/bin/sh -c 'pkill -INT -u $OWNER -f "^$OPT/blink start -c $DIR/blink.toml"; while /usr/bin/tmux -L $NAME has-session -t $NAME 2>/dev/null; do sleep 1; done'
Type=forking
RemainAfterExit=yes
TimeoutStopSec=180

[Install]
WantedBy=multi-user.target
U
systemctl daemon-reload
systemctl enable -q --now "$NAME"

step "Waiting for the server"
INNER=$(sed -n 's/^address = "127.0.0.1:\([0-9]*\)"/\1/p' blink.toml | head -1)
for _ in $(seq 120); do
  ss -ltnH "sport = :$INNER" | grep -q . && break
  sleep 1
done
echo
if ss -ltnH "sport = :$INNER" | grep -q .; then
  echo "${G}${B}Blink is running.${N}"
else
  warn "the server did not come up within 2 minutes (the first start is a slow training run); look at the console: sudo -u $OWNER tmux -L $NAME attach -t $NAME"
fi
cat <<D

  Players join on port $PORT as always. $SLEEP seconds after the last player leaves, the server
  saves and freezes; the next join wakes it up in a fraction of a second.

  Console:     sudo -u $OWNER tmux -L $NAME attach -t $NAME     (leave with Ctrl+B, then D)
  Stop/start:  sudo systemctl stop $NAME / sudo systemctl start $NAME
  Settings:    $DIR/blink.toml
  Undo:        curl -fsSL $RAW/install.sh | sudo bash -s -- --uninstall --name $NAME --dir $DIR

D
