#!/usr/bin/env bash
# Cycle the running TV window's scanline look: off → subtle → heavy → crt → off (SIGUSR1).
set -euo pipefail
. "$(dirname "$0")/lib.sh"
if pid=$(tv_pid); then
  kill -USR1 "$pid" && echo "scanlines: next style"
else
  echo "nerv tv not running"
fi
