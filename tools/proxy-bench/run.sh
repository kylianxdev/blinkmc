#!/bin/bash
# Throughput of Blink's proxy: a fake server streams data to 100 fake players through blink.
#   tools/proxy-bench/run.sh /path/to/blink [threads]
# Prints MB/s and blink's CPU use. Uses ports 25590 (blink) and 25591 (fake server).
set -u
BLINK=$1 THREADS=${2:-1} HERE=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d); trap 'kill $BE $BP 2>/dev/null; rm -rf "$TMP"' EXIT
printf '[public]\naddress = "127.0.0.1:25590"\nversion = "bench"\nprotocol = 777\n[server]\naddress = "127.0.0.1:25591"\ndirectory = "."\ncommand = "sleep 100000"\nwake_on_start = true\nwake_whitelist = false\nsave_before_freeze = false\n[time]\nsleep_after = 100000\n[advanced]\nthreads = %s\n[config]\nversion = "0.2.11"\n' "$THREADS" > "$TMP/blink.toml"
python3 "$HERE/backend.py" & BE=$!
sleep 1
"$BLINK" start -c "$TMP/blink.toml" >/dev/null 2>&1 & BP=$!
until python3 "$HERE/../status-ping.py" 127.0.0.1:25590 2>/dev/null | grep -q '"max": 100'; do sleep 0.5; done
c0=$(awk '{print $14+$15}' /proc/$BP/stat); t0=$(date +%s.%N)
for k in 1 2 3 4; do python3 "$HERE/clients.py" 25 8 > "$TMP/out$k" & done
wait $(jobs -p | grep -vx "$BE" | grep -vx "$BP")
c1=$(awk '{print $14+$15}' /proc/$BP/stat); el=$(echo "$(date +%s.%N) - $t0" | bc)
echo "threads=$THREADS: $(cat "$TMP"/out* | grep -o '[0-9]* MB/s' | awk '{s+=$1} END{print s}') MB/s through blink, blink CPU $(echo "($c1 - $c0) / $el" | bc)%, RSS $(grep VmRSS /proc/$BP/status | awk '{print $2}') kB"
