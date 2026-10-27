#!/bin/bash
# When lazymc freezes the server (SIGSTOP), ask the kernel to move its memory out of the way:
# first compressed into RAM (zswap), the coldest part to the swap file. That memory is then free
# for the rest of the machine, and on a join the server continues within a fraction of a second
# (SIGCONT, pages come back on their own when they are touched).
#
# Usage: blink-freeze-helper.sh <systemd service name>     (runs as root, see blink-freeze-helper@.service)
SERVICE=${1:-minecraft}
CG=/sys/fs/cgroup/system.slice/$SERVICE.service
done_pid=""
while sleep 2; do
  # the server's JVM, found in the service's own cgroup (so several servers don't mix up)
  pid=""
  for p in $(cat $CG/cgroup.procs 2>/dev/null); do
    tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null | grep -q craftbukkit.Main && { pid=$p; break; }
  done
  [ -z "$pid" ] && { done_pid=""; continue; }
  state=$(awk '{print $3}' /proc/$pid/stat 2>/dev/null)
  if [ "$state" != "T" ]; then done_pid=""; continue; fi
  [ "$done_pid" = "$pid" ] && continue
  sleep 5   # lazymc may thaw it again right away
  [ "$(awk '{print $3}' /proc/$pid/stat 2>/dev/null)" = "T" ] || continue
  before=$(cat $CG/memory.current)
  # all but ~64 MB; swappiness=200: prefer anonymous memory (the Java heap) over file cache
  echo "$((before - 64*1024*1024)) swappiness=200" > $CG/memory.reclaim 2>/dev/null
  echo "$SERVICE frozen ($pid): $((before>>20)) MB -> $(( $(cat $CG/memory.current)>>20 )) MB in RAM (zswap $(( $(cat $CG/memory.zswap.current)>>20 )) MB, swap $(( $(cat $CG/memory.swap.current)>>20 )) MB)"
  done_pid=$pid
done
