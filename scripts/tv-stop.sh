#!/usr/bin/env bash
set -euo pipefail
. "$(dirname "$0")/lib.sh"
if pid=$(tv_pid); then
  kill "$pid" && echo "nerv tv stopped"
else
  echo "nerv tv not running"
fi
