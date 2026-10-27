#!/usr/bin/env bash
# Blink installer: your Minecraft server sleeps when nobody plays and wakes up in 0.1 s.
#
#   cd /path/to/your/server        # the folder with paper.jar
#   curl -fsSL https://raw.githubusercontent.com/kylianxdev/blinkmc/main/install.sh | sudo bash
#
# Options (after "bash -s --" when piping, e.g. "| sudo bash -s -- --heap 6G"):
#   --dir PATH       server folder (default: the current folder)
#   --heap SIZE      maximum Java heap, e.g. 6G (default: 60% of the RAM, at most 8G)
#   --swap SIZE      size of the swap file when there is none yet (default: heap + 2G)
#   --name NAME      name of the systemd service (default: minecraft)
#   --sleep SECONDS  freeze after this many seconds without players (default: 30)
#   --uninstall      undo everything for this server (keeps swap and zswap)
#
# What it does: installs Java 26 if needed, puts the patched lazymc and the freeze helper in
# /opt/blinkmc, writes start.sh and lazymc.toml in your server folder (the old files are kept as
# *.before-blink), sets up a swap file and zswap, and runs everything as a systemd service.
set -euo pipefail

REPO=${BLINK_REPO:-kylianxdev/blinkmc}
REF=${BLINK_REF:-main}
RAW="https://raw.githubusercontent.com/$REPO/$REF"
OPT=/opt/blinkmc

DIR=$PWD HEAP="" SWAP="" NAME=minecraft SLEEP=30 UNINSTALL=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dir) DIR=$2; shift ;;
    --heap) HEAP=$2; shift ;;
    --swap) SWAP=$2; shift ;;
    --name) NAME=$2; shift ;;
    --sleep) SLEEP=$2; shift ;;
    --uninstall) UNINSTALL=1 ;;
    -h|--help) sed -n '2,19p' "$0" 2>/dev/null || echo "see $RAW/install.sh"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done

if [ -t 1 ]; then G=$'\e[32m' Y=$'\e[33m' R=$'\e[31m' B=$'\e[1m' N=$'\e[0m'; else G="" Y="" R="" B="" N=""; fi
step() { echo "${G}==>${N} ${B}$*${N}"; }
warn() { echo "${Y}  ! $*${N}"; }
die() { echo "${R}  x $*${N}" >&2; exit 1; }

[ "$(id -u)" = 0 ] || die "run it with sudo"
command -v systemctl >/dev/null || die "Blink needs systemd"
DIR=$(cd "$DIR" && pwd)
UNIT=/etc/systemd/system/$NAME.service

# ---------------------------------------------------------------- uninstall
if [ "$UNINSTALL" = 1 ]; then
  step "Removing Blink from $DIR (service $NAME)"
  systemctl disable --now "$NAME" "blink-freeze-helper@$NAME" 2>/dev/null || true
  rm -f "$UNIT"
  for f in start.sh server.properties lazymc.toml; do
    [ -f "$DIR/$f.before-blink" ] && mv -f "$DIR/$f.before-blink" "$DIR/$f" && echo "  restored $f"
  done
  # Blink's own files that had nothing to restore
  grep -qs "^# Blink:" "$DIR/lazymc.toml" && rm -f "$DIR/lazymc.toml"
  grep -qs "paper.aot" "$DIR/start.sh" && rm -f "$DIR/start.sh"
  rm -f "$DIR"/paper.aot "$DIR"/paper.aot.config "$DIR"/paper.aot.stamp "$DIR"/classpath.txt
  systemctl daemon-reload
  echo "  done. Swap, zswap and /opt/blinkmc are left in place (other servers may use them)."
  echo "  Paper's watchdog settings in spigot.yml stay raised; set timeout-time back to 60 if you like."
  exit 0
fi

# ---------------------------------------------------------------- checks
step "Checking $DIR"
cd "$DIR"
JAR=""
for f in paper.jar *.jar; do
  [ -f "$f" ] || continue
  python3 - "$f" <<'P' 2>/dev/null && { JAR=$f; break; }
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
z.getinfo("META-INF/versions.list"); z.getinfo("META-INF/libraries.list")
P
done
[ -n "$JAR" ] || die "no Paper jar here (Paper, Purpur, Folia and other Paper forks work)"
OWNER=$(stat -c %U "$JAR")
[ "$OWNER" != root ] || warn "the server files belong to root; the server will run as root"
echo "  server jar: $JAR, runs as: $OWNER"

# the public port: from before Blink, else from an earlier Blink install, else server.properties (lazymc moves the
# real server to another port in there, so a second run must not take that one)
PORT=$(sed -n 's/^server-port=//p' server.properties.before-blink 2>/dev/null | tail -1 || true)
[ -n "$PORT" ] || PORT=$(sed -n 's/^address = "0.0.0.0:\([0-9]*\)"/\1/p' lazymc.toml 2>/dev/null | head -1 || true)
[ -n "$PORT" ] || PORT=$(sed -n 's/^server-port=//p' server.properties 2>/dev/null | tail -1 || true)
PORT=${PORT:-25565}
if systemctl is-active -q "$NAME" 2>/dev/null; then
  step "Stopping the running $NAME service"
  systemctl stop "$NAME"
fi
if ss -ltnH "sport = :$PORT" | grep -q .; then
  die "port $PORT is in use. Stop your Minecraft server first, then run this again."
fi
[ "$(uname -m)" = x86_64 ] || die "the prebuilt lazymc is for x86_64; build it from https://github.com/kylianxdev/lazymc/tree/blink"
if ! grep -qs '^eula=true' eula.txt; then
  warn "eula.txt does not say eula=true yet; the server won't start until you accept the Minecraft EULA there"
fi

# ---------------------------------------------------------------- packages
step "Installing what is missing"
JAVA=""
for j in /usr/lib/jvm/*/bin/java; do
  [ -x "$j" ] || continue
  v=$("$j" -XshowSettings:properties -version 2>&1 | sed -n 's/.*java.specification.version = //p')
  [ "${v%%.*}" -ge 26 ] 2>/dev/null && JAVA=$j
done
need=()
command -v tmux >/dev/null || need+=(tmux)
command -v python3 >/dev/null || need+=(python3)
command -v curl >/dev/null || need+=(curl)
[ -n "$JAVA" ] || need+=(openjdk-26-jre-headless)
if [ ${#need[@]} -gt 0 ]; then
  command -v apt-get >/dev/null || die "please install: ${need[*]} (Java 26 or newer)"
  apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${need[@]}" >/dev/null
  [ -n "$JAVA" ] || JAVA=$(ls /usr/lib/jvm/*26*/bin/java | head -1)
fi
echo "  java: $JAVA"

# ---------------------------------------------------------------- Blink files
step "Downloading Blink to $OPT"
mkdir -p "$OPT"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "https://github.com/$REPO/releases/latest/download/lazymc-blink-linux-x64" -o "$tmp/lazymc-blink-linux-x64"
curl -fsSL "https://github.com/$REPO/releases/latest/download/lazymc-blink-linux-x64.sha256" -o "$tmp/lazymc-blink-linux-x64.sha256"
(cd "$tmp" && sha256sum -c --quiet lazymc-blink-linux-x64.sha256) || die "checksum of lazymc does not match"
install -m 755 "$tmp/lazymc-blink-linux-x64" "$OPT/lazymc"
curl -fsSL "$RAW/start.sh" -o "$OPT/start.sh"
curl -fsSL "$RAW/setup/blink-freeze-helper.sh" -o "$OPT/blink-freeze-helper.sh"
curl -fsSL "$RAW/setup/blink-before-freeze.sh" -o "$OPT/blink-before-freeze.sh"
chmod 755 "$OPT/blink-freeze-helper.sh" "$OPT/blink-before-freeze.sh"

# ---------------------------------------------------------------- server folder
step "Setting up the server folder"
if [ -z "$HEAP" ]; then
  ram=$(awk '/MemTotal/ {print int($2/1024/1024)}' /proc/meminfo)
  h=$(( ram * 6 / 10 )); [ $h -gt 8 ] && h=8; [ $h -lt 2 ] && h=2
  HEAP=${h}G
fi
# keep the user's own files once (not the ones an earlier Blink install wrote)
keep() { [ -f "$1" ] && [ ! -f "$1.before-blink" ] && ! grep -qs -e "paper.aot" -e "^# Blink:" "$1" && cp -p "$1" "$1.before-blink"; true; }
keep start.sh; keep server.properties; keep lazymc.toml
sed -e "s#^JAVA=.*#JAVA=\${JAVA:-$JAVA}#" -e "s#^HEAP=.*#HEAP=\${HEAP:-$HEAP}#" -e "s#^JAR=.*#JAR=\${JAR:-$JAR}#" \
  "$OPT/start.sh" > start.sh
chmod 755 start.sh
# a fresh server has no spigot.yml yet: create the two watchdog settings (Spigot adds the rest)
[ -f spigot.yml ] || printf 'settings:\n  timeout-time: 1728000\n  restart-on-crash: false\n' > spigot.yml
chown "$OWNER": start.sh spigot.yml

echo "  unpacking Paper to read its version"
sudo -u "$OWNER" "$JAVA" -Dpaperclip.patchonly=true -jar "$JAR" >/dev/null 2>&1 || die "could not unpack $JAR"
INNER=$((PORT + 1)); while ss -ltnH "sport = :$INNER" | grep -q .; do INNER=$((INNER + 1)); done

JAR="$JAR" PORT=$PORT INNER=$INNER SLEEP=$SLEEP NAME=$NAME OPT=$OPT python3 > lazymc.toml <<'P'
import json, os, re, zipfile
outer = zipfile.ZipFile(os.environ["JAR"])
inner = outer.read("META-INF/versions.list").decode().split("\t")[2].strip()
info = json.loads(zipfile.ZipFile("versions/" + inner).read("version.json"))
brand = "Paper"
m = re.search(r"Brand-Name: (\S+)", zipfile.ZipFile("versions/" + inner).read("META-INF/MANIFEST.MF").decode())
if m: brand = m.group(1)

props = {}
try:
    text = open("server.properties", encoding="latin-1").read()
    for line in text.splitlines():
        if "=" in line and not line.lstrip().startswith("#"):
            k, v = line.split("=", 1)
            props[k.strip()] = v
except OSError:
    pass

def unescape(v):   # Java .properties escapes
    v = re.sub(r"\\u([0-9a-fA-F]{4})", lambda m: chr(int(m.group(1), 16)), v)
    return re.sub(r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), v)

def toml(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'

motd = unescape(props.get("motd", "A Minecraft Server")).split("\n")[0] or "A Minecraft Server"
whitelist = props.get("white-list", "false").strip() == "true"
print(f"""# Blink: freezes the server after {os.environ['SLEEP']} seconds without players, wakes it up when someone joins.
# The server list (MOTD, icon) keeps working while it sleeps. Docs: https://github.com/kylianxdev/blinkmc
[public]
address = "0.0.0.0:{os.environ['PORT']}"
version = {toml(brand + ' ' + info['name'])}
protocol = {info['protocol_version']}

[server]
# lazymc moved the real server here (server-ip and server-port in server.properties)
address = "127.0.0.1:{os.environ['INNER']}"
directory = "."
command = "./start.sh"
freeze_process = true
wake_on_start = true
# save the world first, so nothing is lost if the machine reboots while the server is frozen
before_freeze = "{os.environ['OPT']}/blink-before-freeze.sh {os.environ['NAME']} {os.environ['NAME']}"
start_timeout = 300
stop_timeout = 150
# only players on whitelist.json can wake the server (bots try to join public servers all day)
wake_whitelist = {'true' if whitelist else 'false'}

[time]
sleep_after = {int(os.environ['SLEEP'])}
minimum_online_time = {min(int(os.environ['SLEEP']), 60)}

[motd]
sleeping = {toml(motd + chr(10) + "§7💤 sleeping §8• §ajoin to wake it up")}
starting = {toml(motd + chr(10) + "§e⌛ waking up...")}
stopping = {toml(motd + chr(10) + "§7💤 going to sleep...")}
from_server = true

[join]
methods = ["hold", "kick"]

[join.hold]
timeout = 40

[join.kick]
starting = "§aThe server is waking up!§r\\n\\n§7Try again in §e10 seconds§7."
stopping = "§7The server is going to sleep...§r\\n\\n§7Try again in §e10 seconds§7."

[advanced]
rewrite_server_properties = true

[config]
version = "0.2.11"
""")
P
chown "$OWNER": lazymc.toml
WL=$(sed -n 's/^wake_whitelist = //p' lazymc.toml)
[ "$WL" = true ] || warn "your whitelist is off, so anyone (also bots) can wake the server. Turn it on, or set wake_whitelist = true in lazymc.toml with a whitelist.json of your players."

# ---------------------------------------------------------------- memory: swap and zswap
step "Setting up swap and zswap"
if ! swapon --show=NAME --noheadings | grep -q .; then
  [ -n "$SWAP" ] || SWAP=$(( ${HEAP%G} + 2 ))G
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

# ---------------------------------------------------------------- services
step "Installing the services"
cat > "$UNIT" <<U
[Unit]
Description=Minecraft server $NAME (Blink: freezes when nobody plays, wakes up on join)
After=network-online.target
Wants=network-online.target blink-freeze-helper@$NAME.service

[Service]
User=$OWNER
WorkingDirectory=$DIR
ExecStart=/usr/bin/tmux -L $NAME new-session -d -s $NAME $OPT/lazymc start -c $DIR/lazymc.toml
# SIGINT to lazymc: it thaws a frozen server, lets it save and stop, then quits
ExecStop=/bin/sh -c 'kill -INT \$(pgrep -u $OWNER -f "^$OPT/lazymc start -c $DIR/lazymc.toml") 2>/dev/null; true'
ExecStop=/bin/sh -c 'while /usr/bin/tmux -L $NAME has-session -t $NAME 2>/dev/null; do sleep 1; done'
Type=forking
RemainAfterExit=yes
TimeoutStopSec=180

[Install]
WantedBy=multi-user.target
U
cat > /etc/systemd/system/blink-freeze-helper@.service <<U
[Unit]
Description=Blink: move the memory of the frozen Minecraft server %i to zswap and swap
After=%i.service

[Service]
ExecStart=$OPT/blink-freeze-helper.sh %i
Restart=always

[Install]
WantedBy=multi-user.target
U
systemctl daemon-reload
systemctl enable -q --now "blink-freeze-helper@$NAME"
systemctl enable -q --now "$NAME"

# ---------------------------------------------------------------- done
step "Waiting for the server"
for _ in $(seq 120); do
  ss -ltnH "sport = :$INNER" | grep -q . && break
  sleep 1
done
if ss -ltnH "sport = :$INNER" | grep -q .; then
  echo
  echo "${G}${B}Blink is running.${N}"
else
  echo
  warn "the server did not come up within 2 minutes; look at the console: sudo -u $OWNER tmux -L $NAME attach -t $NAME"
fi
cat <<D

  Players join on port $PORT as always. $SLEEP seconds after the last player leaves, the server
  saves and freezes; the next join wakes it up in a fraction of a second.

  Console:     sudo -u $OWNER tmux -L $NAME attach -t $NAME     (leave with Ctrl+B, then D)
  Stop/start:  sudo systemctl stop $NAME / sudo systemctl start $NAME
  Settings:    $DIR/lazymc.toml
  Undo:        curl -fsSL $RAW/install.sh | sudo bash -s -- --uninstall --name $NAME --dir $DIR

D
