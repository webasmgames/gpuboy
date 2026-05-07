#!/usr/bin/env bash
set -euo pipefail

if [ -d "tests/roms" ] && [ -n "$(ls -A tests/roms 2>/dev/null)" ]; then
    echo "tests/roms/ already populated, skipping download."
    exit 0
fi

mkdir -p tests/roms

TMP=$(mktemp --suffix=.zip)
curl -L -o "$TMP" "https://github.com/c-sp/game-boy-test-roms/releases/download/v7.0/game-boy-test-roms-v7.0.zip"
unzip -q "$TMP" -d tests/roms/
rm "$TMP"
echo "Test ROMs downloaded to tests/roms/"
