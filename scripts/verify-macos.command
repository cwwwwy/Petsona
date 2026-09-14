#!/usr/bin/env bash
set -uo pipefail

PETSONA_SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
bash "$PETSONA_SCRIPT_DIR/verify-macos.sh"
PETSONA_EXIT_CODE=$?

printf '\n'
if [[ $PETSONA_EXIT_CODE -eq 0 ]]; then
  printf 'macOS verification passed.\n'
else
  printf 'macOS verification failed with exit code %s.\n' "$PETSONA_EXIT_CODE"
fi

read -r -p "Press Return to close..." _
exit "$PETSONA_EXIT_CODE"
