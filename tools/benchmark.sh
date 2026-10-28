#!/bin/bash
# Blink benchmark: cold starts, memory with simulated players, and freeze/wake for several freeze delays.
# Runs against a Blink 2 install (install.sh --name <service>) on a COPY of a real world, while nobody plays.
#
#   tools/benchmark.sh <service> <server folder> <public port> <name on whitelist.json> > results.csv
#
# Players are simulated with /forceload: each "player" loads the 25 x 25 chunks a player with view
# distance 12 would load, centered on one of the biggest region files of the overworld (where people
# actually built). The service's blink.toml is changed during the run and put back at the end.
set -u
SERVICE=$1 DIR=$2 PORT=$3 NAME=$4
HERE=$(cd "$(dirname "$0")" && pwd)
CG=/sys/fs/cgroup/system.slice/$SERVICE.service
TOML=$DIR/blink.toml
cp "$TOML" "$TOML.bench-backup"
trap 'cp "$TOML.bench-backup" "$TOML"; rm -f "$TOML.bench-backup"; sudo systemctl restart "$SERVICE"' EXIT

out() { echo "$*"; echo "# $*" >&2; }
console() { tmux -L "$SERVICE" send-keys -t "$SERVICE" "$1" Enter; }
jvm() { for p in $(cat $CG/cgroup.procs); do tr '\0' ' ' < /proc/$p/cmdline | grep -q craftbukkit.Main && echo $p; done | head -1; }
rss_mb() { echo $(( $(ps -o rss= -p "$(jvm)") / 1024 )); }
cg_mb() { echo $(( $(cat $CG/memory.current) >> 20 )); }
state() { awk '{print $3}' /proc/"$(jvm)"/stat 2>/dev/null; }
set_toml() { sed -i -E "s/^($1 = ).*/\1$2/" "$TOML"; }

wait_done() {   # waits for a new latest.log that says Done, prints Paper's number
  local inode=$1
  until [ "$(stat -c %i "$DIR/logs/latest.log" 2>/dev/null)" != "$inode" ] && grep -q "Done (" "$DIR/logs/latest.log" 2>/dev/null; do sleep 0.1; done
  grep -o 'Done ([0-9.]*s)' "$DIR/logs/latest.log" | grep -o '[0-9.]*'
}
restart() {     # restart the service, print wall seconds and Paper's seconds
  local inode t0 t1 paper
  inode=$(stat -c %i "$DIR/logs/latest.log" 2>/dev/null || echo none)
  sudo systemctl stop "$SERVICE"
  t0=$(date +%s.%N); sudo systemctl start "$SERVICE"
  paper=$(wait_done "$inode"); t1=$(date +%s.%N)
  echo "$(echo "$t1 - $t0" | bc) $paper"
}
wake() { python3 "$HERE/status-ping.py" "127.0.0.1:$PORT" --join --name "$NAME" | grep -o '[0-9.]* s' | grep -o '[0-9.]*'; }
wait_frozen() { until [ "$(state)" = T ]; do sleep 0.5; done; }
wait_reclaimed() { local n=$1; until [ "$(tmux -L "$SERVICE" capture-pane -p -t "$SERVICE" -S -2000 | grep -c "Frozen server's memory")" -gt "$n" ]; do sleep 1; done; }
reclaims() { tmux -L "$SERVICE" capture-pane -p -t "$SERVICE" -S -2000 | grep -c "Frozen server's memory"; }

# the "players": centers (in chunks) of the 4 biggest overworld region files
mapfile -t CENTERS < <(find "$DIR/world" -path "*overworld*" -name "r.*.mca" -printf "%s %f\n" | sort -rn | head -4 |
  awk '{split($2, r, "."); print r[2]*32+16, r[3]*32+16}')
players() {     # forceload (add|remove) the chunks of the first n players
  local action=$1 n=$2 i cx cz x0 x1 z
  for ((i = 0; i < n; i++)); do
    read -r cx cz <<< "${CENTERS[$i]}"
    x0=$(( (cx - 12) * 16 )); x1=$(( (cx + 12) * 16 + 15 ))
    for z in -12:-4 -3:4 5:12; do   # 3 strips, a command may hold at most 256 chunks
      console "forceload $action $x0 $(( (cz + ${z%:*}) * 16 )) $x1 $(( (cz + ${z#*:}) * 16 + 15 ))"
    done
  done
}

echo "test,detail,value,unit"

# 1. cold starts (no freezing during this part)
set_toml sleep_after 3600; set_toml minimum_online_time 60
restart >/dev/null   # first start after a training run builds the AOT cache
for i in 1 2 3; do
  read -r wall paper <<< "$(restart)"
  out "cold_start,run $i wall,$wall,s"; out "cold_start,run $i paper,$paper,s"
done

# 2. memory with 0, 1, 2 and 4 simulated players
sleep 70
out "memory_awake,0 players,$(rss_mb),MB"
for n in 1 2 4; do
  players add $n; sleep 45
  out "memory_awake,$n players,$(rss_mb),MB"
done
players remove 4; sleep 150   # periodic GC gives memory back after a minute without other GCs
out "memory_awake,after players left (150 s),$(rss_mb),MB"

# 3. freeze delays: after a "session" of 4 players, how much stays in RAM when frozen, and how fast it wakes
for delay in 10 30 60 300; do
  set_toml sleep_after $delay; set_toml minimum_online_time $(( delay < 60 ? delay : 60 ))
  restart >/dev/null
  for i in 1 2 3; do
    players add 4; sleep 45; players remove 4
    n=$(reclaims); t0=$(date +%s.%N)
    wait_frozen; froze=$(echo "$(date +%s.%N) - $t0" | bc)
    wait_reclaimed "$n"
    out "freeze_${delay}s,run $i seconds until frozen,$froze,s"
    out "freeze_${delay}s,run $i frozen in RAM,$(cg_mb),MB"
    sleep 20
    out "freeze_${delay}s,run $i wake,$(wake),s"
  done
done
