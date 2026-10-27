#!/bin/bash
# One time setup for the "frozen" state: a swap file, zswap (compressed RAM in front of it) and
# the freeze helper service. Run as root. Safe to run again.
#   SWAP_SIZE=12G sudo setup/enable-sleep.sh
set -eu
SWAP_SIZE=${SWAP_SIZE:-12G}
HERE=$(cd "$(dirname "$0")/.." && pwd)

# 1. swap file (the frozen heap needs somewhere to go)
if ! swapon --show=NAME --noheadings | grep -q .; then
  fallocate -l "$SWAP_SIZE" /swapfile
  chmod 600 /swapfile
  mkswap /swapfile >/dev/null
  swapon /swapfile
  grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
  echo "swap file of $SWAP_SIZE created"
fi

# 2. low swappiness for the running server, no read ahead for swap (pointless with zswap)
cat > /etc/sysctl.d/90-blink.conf <<'S'
vm.swappiness=10
vm.page-cluster=0
S
sysctl -q -p /etc/sysctl.d/90-blink.conf

# 3. zswap with lz4: frozen pages are kept compressed in RAM, so waking up does not wait for the disk
modprobe lz4 2>/dev/null || true
echo lz4 > /etc/modules-load.d/blink-zswap.conf
cat > /etc/tmpfiles.d/blink-zswap.conf <<'T'
w /sys/module/zswap/parameters/compressor - - - - lz4
w /sys/module/zswap/parameters/max_pool_percent - - - - 25
w /sys/module/zswap/parameters/enabled - - - - Y
T
systemd-tmpfiles --create /etc/tmpfiles.d/blink-zswap.conf

# 4. the helper that moves a frozen server's memory out of the way
sed "s#/opt/blinkmc#$HERE#" "$HERE/systemd/blink-freeze-helper.service" > /etc/systemd/system/blink-freeze-helper.service
systemctl daemon-reload
systemctl enable --now blink-freeze-helper >/dev/null
echo "zswap: $(cat /sys/module/zswap/parameters/enabled) ($(cat /sys/module/zswap/parameters/compressor)), helper: $(systemctl is-active blink-freeze-helper)"

cat <<'N'

Two settings in your server folder are still up to you (see the README):
  spigot.yml:               settings.timeout-time: 1728000   and   restart-on-crash: false
  config/paper-global.yml:  watchdog.early-warning-delay: 1728000000
Without them Paper's watchdog thinks a server that was frozen for a while has hung, and kills it.
N
