# Blink's Paper launcher, built into the blink binary (lazymc.toml: command = "blink-paper ...").
# Runs with sh in the server folder. Settings come as environment variables from that command line,
# for example: command = "blink-paper HEAP=6G JAR=paper.jar"
#
# Starts a Paper server as fast as possible (about 4 s instead of about 13 s).
#
#  1. Paper is started directly instead of through paperclip, so all of its classes come from the
#     normal classpath and fit in Java's AOT cache.
#  2. The AOT cache (paper.aot) holds the loaded and linked classes, lambdas and method profiles.
#  3. No -XX:+AlwaysPreTouch: touching the whole heap up front costs seconds.
#
# The cache belongs to exactly one Paper build and one Java build. After an update this script
# notices it: the first start is a normal (slow) training run, the start after it builds the cache.
#
# Needs Java 26 or newer (see the README for why).

JAVA=${JAVA:-/usr/lib/jvm/java-26-openjdk-amd64/bin/java}
HEAP=${HEAP:-8G}
# start small and give memory back when it is quiet: G1 runs a GC after 15 s without other GCs and
# returns what it does not need, so a server nobody plays on is small by the time it is frozen
HEAP_MIN=${HEAP_MIN:-256M}
# Aikar's flags, without AlwaysPreTouch (slow start) and ParallelRefProcEnabled (deprecated in 26)
FLAGS="-Xms$HEAP_MIN -Xmx$HEAP -XX:G1PeriodicGCInterval=15000 -XX:MinHeapFreeRatio=15 -XX:MaxHeapFreeRatio=40 -XX:+DisableExplicitGC -XX:+PerfDisableSharedMem \
  -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1HeapRegionSize=8M -XX:G1HeapWastePercent=5 \
  -XX:G1MaxNewSizePercent=40 -XX:G1MixedGCCountTarget=4 -XX:G1MixedGCLiveThresholdPercent=90 \
  -XX:G1NewSizePercent=30 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:G1ReservePercent=20 \
  -XX:InitiatingHeapOccupancyPercent=15 -XX:MaxGCPauseMillis=200 -XX:MaxTenuringThreshold=1 \
  -XX:SurvivorRatio=32 -XX:+UseTransparentHugePages"
MAIN=org.bukkit.craftbukkit.Main
JAR=${JAR:-paper.jar}

# A frozen server sees a gap of minutes or days when it wakes up. Paper's watchdog would call that a
# hang and kill it, and restart-on-crash would then start a second server outside Blink.
if [ -f spigot.yml ]; then
  sed -i -E 's/^(  timeout-time:).*/\1 1728000/; s/^(  restart-on-crash:).*/\1 false/' spigot.yml
fi
if [ -f config/paper-global.yml ]; then
  sed -i -E 's/^(  early-warning-delay:).*/\1 1728000000/' config/paper-global.yml
  # Blink passes on the players' real IP addresses with the PROXY protocol (send_proxy_v2)
  if [ "${PROXY_PROTOCOL:-false}" = true ]; then
    sed -i -E 's/^(  proxy-protocol:).*/\1 true/' config/paper-global.yml
  fi
fi

# New paper.jar or new Java: unpack Paper, write the classpath again, throw the cache away.
# (stat instead of "java -version": starting a JVM just to compare versions costs half a second)
STAMP="$(stat -c %s-%Y "$JAR") $(stat -c %s-%Y "$(dirname "$JAVA")/../lib/server/libjvm.so")"
if [ "$(cat paper.aot.stamp 2>/dev/null)" != "$STAMP" ]; then
  "$JAVA" -Dpaperclip.patchonly=true -jar "$JAR" || exit 1
  JAR="$JAR" python3 - <<'P' || exit 1
import os, zipfile
z = zipfile.ZipFile(os.environ["JAR"])
version = z.read("META-INF/versions.list").decode().split("\t")[2].strip()
libs = [l.split("\t")[2].strip() for l in z.read("META-INF/libraries.list").decode().splitlines() if l.strip()]
open("classpath.txt", "w").write(":".join(["versions/" + version] + ["libraries/" + l for l in libs]))
P
  rm -f paper.aot paper.aot.config
  echo "$STAMP" > paper.aot.stamp
fi
CP="$(cat classpath.txt)"

# A finished training run left paper.aot.config: build the cache from it now (a few seconds).
# AOTStreamableObjects: without it Minecraft's big MethodType table does not fit
# ("Cannot archive the sub-graph ... size 262160") and the JVM writes an empty cache.
if [ -s paper.aot.config ] && [ ! -s paper.aot ]; then
  "$JAVA" $FLAGS -XX:+UnlockDiagnosticVMOptions -XX:+AOTStreamableObjects -XX:AOTMode=create \
    -XX:AOTConfiguration=paper.aot.config -XX:AOTCache=paper.aot -cp "$CP" >/dev/null 2>&1 || rm -f paper.aot
  rm -f paper.aot.config
fi

if [ -s paper.aot ]; then
  exec "$JAVA" $FLAGS -XX:AOTCache=paper.aot -cp "$CP" $MAIN --nogui
fi

# Training run: Java writes down what Paper uses and saves it when the server stops; the next start
# turns it into the cache. "exec", so Blink's signals (freeze, thaw, stop) reach Java itself.
exec "$JAVA" $FLAGS -XX:AOTMode=record -XX:AOTConfiguration=paper.aot.config -cp "$CP" $MAIN --nogui
