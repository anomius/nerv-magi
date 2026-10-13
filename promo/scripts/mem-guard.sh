#!/usr/bin/env bash
# Wait until the machine has room for a heavy job: MemAvailable above MIN_GB (default 3) GB.
# Gives up after MAX_WAIT seconds (default 900) with exit 1. Used before every Remotion render and encode:
# this machine has 15 GB and other agents may be running FluidSynth or builds.
#   mem-guard.sh [min_gb] [max_wait_s]
set -euo pipefail
min=${1:-3}; wait=${2:-900}; t=0
avail() { awk '/MemAvailable/ { printf "%.1f", $2 / 1048576 }' /proc/meminfo; }
while awk -v a="$(avail)" -v m="$min" 'BEGIN { exit !(a < m) }'; do
  (( t == 0 )) && echo "mem-guard: $(avail) GB available, waiting for > $min GB" >&2
  (( t >= wait )) && { echo "mem-guard: still $(avail) GB after ${wait}s, giving up" >&2; exit 1; }
  sleep 10; t=$((t + 10))
done
echo "mem-guard: $(avail) GB available" >&2
