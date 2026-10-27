#!/bin/bash
# Restarts the server with the given start script and measures the time until "Done".
# Run it in the server folder, while nobody is online:  tools/measure-startup.sh ./start.sh
# It uses a tmux session called "minecraft" (stops the one that is running).
set -u
tmux has-session -t minecraft 2>/dev/null && {
  tmux send-keys -t minecraft stop Enter
  while tmux has-session -t minecraft 2>/dev/null; do sleep 0.5; done
}
log=logs/latest.log
inode=$(stat -c %i "$log" 2>/dev/null || echo none)
t0=$(date +%s.%N)
tmux new-session -d -s minecraft "$1"
# Paper starts a new latest.log, so wait for a new file that says "Done"
until [ "$(stat -c %i "$log" 2>/dev/null)" != "$inode" ] && grep -q "Done (" "$log" 2>/dev/null; do sleep 0.1; done
t1=$(date +%s.%N)
echo "$1: $(echo "$t1 - $t0" | bc) s until Done, Paper says $(grep -o 'Done ([0-9.]*s)' "$log")"
