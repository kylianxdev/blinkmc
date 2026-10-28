#!/usr/bin/env bash
# Blink installer: your Minecraft server sleeps when nobody plays and wakes up when someone joins.
#
#   cd /path/to/your/server        # paper.jar, a Fabric or vanilla jar, or Forge's run.sh
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
#   --uninstall      undo everything for this server (also the system settings, if no other
#                    server uses Blink; a swap file stays, see the message)
#   BLINK_BINARY=/path/to/blink   use a blink you built yourself instead of the release
#
# What the full install (Linux, root) changes, all undone by --uninstall:
#   * /opt/blinkmc/blink and a systemd service ($NAME) that runs it in tmux
#   * Java 26 (apt) if there is no Java 26 yet, tmux and curl if missing
#   * a swap file if there is no swap (--swap), zswap with lz4, vm.swappiness=10, vm.page-cluster=0
#   * in the server folder: blink.toml; when Blink starts the server, the port in server.properties
#     (the server moves behind Blink), Paper's watchdog and its PROXY protocol setting
# Running it again updates Blink and keeps your blink.toml.
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
    -h|--help) sed -n '2,29p' "$0" 2>/dev/null || echo "see $RAW/install.sh"; exit 0 ;;
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

eula_note() {
  grep -qs '^eula=true' "$DIR/eula.txt" \
    || warn "accept the Minecraft EULA first (https://aka.ms/MinecraftEULA): put eula=true in $DIR/eula.txt"
}

# blink.toml: written once, kept when this runs again (an update)
write_config() {   # $@: how to run blink
  if [ -f "$DIR/blink.toml" ]; then
    step "Keeping your blink.toml (delete it and run this again for a fresh one)"
    return
  fi
  step "Writing blink.toml"
  local args=(--sleep "$SLEEP" --threads "$THREADS")
  [ -n "$HEAP" ] && args+=(--heap "$HEAP")
  "$@" init "${args[@]}"
}

# ---------------------------------------------------------------- portable: no root, no changes
if [ "$PORTABLE" = 1 ]; then
  if [ "$UNINSTALL" = 1 ]; then
    step "Removing Blink from $DIR"
    if [ -x "$DIR/blink" ] && [ -f "$DIR/blink.toml" ]; then "$DIR/blink" restore -c "$DIR/blink.toml"; fi
    rm -f "$DIR/blink" "$DIR/blink.toml"
    echo "  done. Start your server the way you did before Blink."
    exit 0
  fi
  step "Putting blink in $DIR"
  pgrep -f "^$DIR/blink start" >/dev/null 2>&1 && die "Blink is running in $DIR; stop it first (Ctrl+C or type stop), then run this again"
  download_blink "$DIR/blink"
  echo "  $("$DIR/blink" --version)"
  write_config "$DIR/blink"
  cat <<D

${G}${B}Done.${N} Start your server with Blink from now on:

  cd $DIR && ./blink start

  Paper starts fastest with Java 26 (the newest Java on your machine is used; set JAVA= in
  blink.toml to choose). Keep it running in tmux or screen if you close the terminal. Stop it
  with Ctrl+C or by typing stop: the server saves and stops, also while it is frozen.
  Undo:  curl -fsSL $RAW/install.sh | bash -s -- --portable --uninstall --dir $DIR

D
  eula_note
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
  # the server's own port, watchdog and PROXY protocol setting back; your other edits stay
  if [ -x "$OPT/blink" ] && [ -f "$DIR/blink.toml" ]; then
    sudo -u "$(stat -c %U "$DIR/blink.toml")" "$OPT/blink" restore -c "$DIR/blink.toml" || warn "blink restore failed; check server.properties"
  fi
  rm -f "$DIR"/blink "$DIR"/blink.toml
  systemctl daemon-reload
  # the system settings, unless another server still uses Blink
  if ! grep -qs "$OPT/blink start" /etc/systemd/system/*.service; then
    rm -f /etc/sysctl.d/90-blink.conf /etc/modules-load.d/blink-zswap.conf /etc/tmpfiles.d/blink-zswap.conf
    rm -rf "$OPT"
    sysctl -q --system >/dev/null 2>&1 || true
    echo "  removed Blink's system settings and $OPT"
    grep -qs '^/swapfile ' /etc/fstab && echo "  the swap file stays (other programs may use it). Remove it with: sudo swapoff /swapfile && sudo sed -i '\\|^/swapfile |d' /etc/fstab && sudo rm /swapfile"
  else
    echo "  other servers still use Blink: kept $OPT and the system settings"
  fi
  echo "  done. Start your server the way you did before Blink."
  exit 0
fi

step "Checking $DIR"
FILE=$(ls -1 paper.jar *.jar run.sh 2>/dev/null | head -1 || true)
[ -n "$FILE" ] || die "no Minecraft server here: run this in the folder with paper.jar, a Fabric or vanilla jar, or Forge's run.sh"
PAPER=0
for j in paper.jar *.jar; do   # Paper's launcher lists (zip file names are stored as plain text)
  [ -f "$j" ] && grep -qa 'META-INF/versions.list' "$j" && { PAPER=1; break; }
done
OWNER=$(stat -c %U "$FILE")
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
eula_note

step "Installing what is missing"
# the newest Java on the machine, and its version
JAVA="" JV=0
for j in /usr/lib/jvm/*/bin/java; do
  [ -x "$j" ] || continue
  v=$("$j" -XshowSettings:properties -version 2>&1 | sed -n 's/.*java.specification.version = //p')
  v=${v%%.*}
  [ "${v:-0}" -gt "$JV" ] 2>/dev/null && JAVA=$j JV=$v
done
need=()
command -v tmux >/dev/null || need+=(tmux)
command -v curl >/dev/null || need+=(curl)
if [ ${#need[@]} -gt 0 ]; then
  command -v apt-get >/dev/null || die "please install: ${need[*]}"
  apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${need[@]}" >/dev/null
fi
# Paper starts in 4 s instead of 13 with Java 26 (the AOT cache); freezing works with any Java
if [ "$PAPER" = 1 ] && [ "$JV" -lt 26 ] && command -v apt-get >/dev/null; then
  apt-get update -qq || true
  if DEBIAN_FRONTEND=noninteractive apt-get install -y -qq openjdk-26-jre-headless >/dev/null 2>&1; then
    JAVA=$(ls /usr/lib/jvm/*26*/bin/java | head -1) JV=26
  fi
fi
if [ -z "$JAVA" ]; then
  command -v java >/dev/null || die "no Java found. Install the Java your Minecraft version needs (Java 26 for the fastest Paper starts), for example from https://adoptium.net/installation/linux/"
  JAVA=$(readlink -f "$(command -v java)")
fi
echo "  java: $JAVA"
[ "$PAPER" = 1 ] && [ "$JV" -lt 26 ] && warn "Java 26 is not in your package sources: Paper starts without the AOT cache (about 13 s cold instead of 4 s; a frozen server still wakes up in under a second). Temurin 26: https://adoptium.net/installation/linux/"

step "Downloading blink to $OPT"
mkdir -p "$OPT"
download_blink "$OPT/blink"
echo "  $("$OPT/blink" --version)"

step "Setting up the server folder"
cleanup_blink1
# the port Blink should listen on, also when server.properties was already rewritten
[ -f server.properties.before-blink ] || { [ -f server.properties ] && cp -p server.properties server.properties.before-blink; }
sed -i "s/^server-port=.*/server-port=$PORT/" server.properties.before-blink 2>/dev/null || true
write_config sudo -u "$OWNER" env JAVA_HOME="$(dirname "$(dirname "$JAVA")")" "$OPT/blink"
"$OPT/blink" config test -c blink.toml || die "blink.toml has an error (above)"

step "Setting up swap and zswap"
if ! swapon --show=NAME --noheadings | grep -q .; then
  h=$(sed -n 's/.*-\{0,1\}\(HEAP=\|Xmx\)\([0-9]*\)G.*/\2/p' blink.toml | head -1); h=${h:-4}
  [ -n "$SWAP" ] || SWAP=$(( h + 2 ))G
  avail=$(df --output=avail -BG / | tail -1 | tr -dc 0-9)
  [ "${SWAP%G}" -lt $(( avail / 2 )) ] || die "not enough disk space for a ${SWAP} swap file (use --swap)"
  if [ "$(stat -f -c %T /)" = btrfs ]; then
    btrfs filesystem mkswapfile --size "$SWAP" /swapfile >/dev/null   # no copy on write, as swap needs
  else
    fallocate -l "$SWAP" /swapfile 2>/dev/null || dd if=/dev/zero of=/swapfile bs=1M count=$(( ${SWAP%G} * 1024 )) status=none
    chmod 600 /swapfile && mkswap -q /swapfile
  fi
  swapon /swapfile
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
modprobe lz4 2>/dev/null || true
echo lz4 > /etc/modules-load.d/blink-zswap.conf
cat > /etc/tmpfiles.d/blink-zswap.conf <<'T'
w /sys/module/zswap/parameters/compressor - - - - lz4
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
# Blink runs as $OWNER and moves a frozen server's memory out of RAM itself (memory.reclaim).
# Older kernels (before 5.19) or no zswap: those files don't exist, Blink then lets the kernel do it.
ExecStartPre=+/bin/sh -c 'chown $OWNER /sys/fs/cgroup/system.slice/%n/memory.reclaim /sys/fs/cgroup/system.slice/%n/memory.zswap.max 2>/dev/null || true'
# in tmux, so the console stays reachable: sudo -u $OWNER tmux -L $NAME attach -t $NAME
ExecStart=/usr/bin/tmux -L $NAME new-session -d -s $NAME $OPT/blink start -c $DIR/blink.toml
# SIGINT to blink: it thaws a frozen server, lets it save and stop, then quits
ExecStop=/bin/sh -c 'pkill -INT -u $OWNER -f "^$OPT/blink start -c $DIR/blink.toml"; while /usr/bin/tmux -L $NAME has-session -t $NAME 2>/dev/null; do sleep 1; done'
Type=forking
# if Blink stops by itself (a crash, or stop typed in the console), start it again
Restart=always
RestartSec=5
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
  saves and freezes; the next join has them back in the world within a second.

  Console:     sudo -u $OWNER tmux -L $NAME attach -t $NAME     (leave with Ctrl+B, then D)
  Stop/start:  sudo systemctl stop $NAME / sudo systemctl start $NAME
               (stop typed in the console restarts it after 5 s; systemctl stop is for good)
  Settings:    $DIR/blink.toml
  Undo:        curl -fsSL $RAW/install.sh | sudo bash -s -- --uninstall --name $NAME --dir $DIR

D
