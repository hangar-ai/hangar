#!/usr/bin/env bash
# The build machine's own safety net, started by the instance's onstart command and after provisioning.
# There is no API key on the box, so it cannot destroy itself: after state/idle-min minutes (60) without
# contact from the PC it stops its build service instead. Running builds are killed and new ones refused
# (state/retired), and the PC's reaper, the next call, or the GitHub reaper (.github/workflows/rbuild-reaper.yml)
# destroys the instance. A run in progress is contact: it streams to the PC for as long as it lasts.
exec 9>/root/hb/state/watchdog.lock
flock -n 9 || exit 0
S=/root/hb/state
touch "$S/contact"
while sleep 60; do
  idle=$(cat "$S/idle-min" 2>/dev/null || echo 60)
  if pgrep -f '/root/hb/bin/remote.py (run|seed-out)' >/dev/null; then
    touch "$S/contact"
    continue
  fi
  now=$(date +%s)
  last=$(stat -c %Y "$S/contact" 2>/dev/null || echo 0)
  if [ $((now - last)) -ge $((idle * 60)) ] && [ ! -e "$S/retired" ]; then
    echo "$(date -Is) no contact from the PC for $(((now - last) / 60)) min: build service stopped" >>"$S/watchdog.log"
    touch "$S/retired"
    pkill -TERM -x cargo; pkill -TERM -x rustc; pkill -TERM -x clippy-driver
    sleep 10
    pkill -KILL -x cargo; pkill -KILL -x rustc; pkill -KILL -x clippy-driver
  fi
done
