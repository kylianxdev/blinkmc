#!/bin/bash
# Runs in the server folder right before lazymc freezes the server (lazymc.toml: before_freeze).
# Makes Paper save the world and waits for it, so a frozen server has nothing unsaved: if the
# machine reboots while it is frozen, nothing is lost.
#   blink-before-freeze.sh <tmux socket> <tmux session>
SOCKET=${1:-default} SESSION=${2:-minecraft} LOG=logs/latest.log
lines=$(wc -l < "$LOG")
tmux -L "$SOCKET" send-keys -t "$SESSION" "save-all" Enter
for _ in $(seq 150); do   # at most 30 s
  tail -n +"$((lines + 1))" "$LOG" | grep -q "Saved the game" && exit 0
  sleep 0.2
done
exit 1
