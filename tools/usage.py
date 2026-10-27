"""How much Blink is really used, from GitHub's own counters (run with a gh login that can push).

    python3 tools/usage.py                 # print the numbers
    python3 tools/usage.py --save FILE     # also add today's numbers to FILE (JSON), e.g. from cron

- installs: downloads of the blink binaries (every install and update downloads one)
- active servers: downloads of releases/usage/active, which every running Blink fetches once a day,
  so the growth per day is about the number of servers running Blink (see usage_stats)
- traffic: repository views, clones and where visitors come from (GitHub keeps 14 days, hence --save)
"""
import json
import subprocess
import sys
import time

REPO = "kylianxdev/blinkmc"


def api(path):
    out = subprocess.run(["gh", "api", f"repos/{REPO}{path}"], capture_output=True, text=True)
    return json.loads(out.stdout) if out.returncode == 0 else None


repo = api("") or {}
releases = api("/releases?per_page=100") or []
installs, active = 0, 0
per_platform = {}
for release in releases:
    for asset in release.get("assets", []):
        name, count = asset["name"], asset["download_count"]
        if release["tag_name"] == "usage" and name == "active":
            active += count
        elif (name.startswith("blink-") or name.startswith("lazymc-blink")) and not name.endswith((".sha256", ".patch")):
            installs += count
            platform = name.replace("lazymc-blink-", "").replace("blink-", "")
            per_platform[platform] = per_platform.get(platform, 0) + count
views = api("/traffic/views") or {}
clones = api("/traffic/clones") or {}
referrers = api("/traffic/popular/referrers") or []

today = {
    "date": time.strftime("%Y-%m-%d"),
    "stars": repo.get("stargazers_count"),
    "forks": repo.get("forks_count"),
    "watchers": repo.get("subscribers_count"),
    "installs": installs,
    "installs_per_platform": per_platform,
    "active_pings_total": active,
    "views_14d": views.get("count"),
    "visitors_14d": views.get("uniques"),
    "clones_14d": clones.get("count"),
    "referrers": {r["referrer"]: r["uniques"] for r in referrers},
}

history = []
if "--save" in sys.argv:
    path = sys.argv[sys.argv.index("--save") + 1]
    try:
        history = json.load(open(path))
    except (OSError, ValueError):
        history = []
    history = [h for h in history if h["date"] != today["date"]] + [today]
    json.dump(history, open(path, "w"), indent=1)

print(f"Blink usage on {today['date']}")
print(f"  stars {today['stars']}, forks {today['forks']}, watchers {today['watchers']}")
print(f"  installs (binary downloads): {installs}  {per_platform}")
if len(history) >= 2:
    print(f"  active servers (pings yesterday to today): {active - history[-2]['active_pings_total']}")
else:
    print(f"  active-server pings so far: {active} (run daily with --save to see servers per day)")
print(f"  last 14 days: {today['views_14d']} views by {today['visitors_14d']} visitors, {today['clones_14d']} clones")
if today["referrers"]:
    print("  visitors come from: " + ", ".join(f"{k} ({v})" for k, v in today["referrers"].items()))
