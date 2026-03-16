#!/usr/bin/env bash
set -euo pipefail

TOOL="$1"
MODE="${2:-}"

if ! command -v "$TOOL" &>/dev/null; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  echo "${TOOL} not installed. Run: ${SCRIPT_DIR}/install.sh" >&2
  exit 0
fi

case "$MODE" in
  check)
    "$TOOL" check .
    ;;
  *)
    "$TOOL" edit
    ;;
esac
