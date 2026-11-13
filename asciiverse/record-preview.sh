#!/bin/sh
# Records asciiverse/settl.cast, the catalog preview: four Computer players
# (settl --demo) at 130x60, the game's own minimum for its board. The
# catalog crops it to 100x30 from the centre (hub/RECORDING.md). Needs
# hub/tools/castrec.py beside this repo (../../hub) and a release build.
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
HOME="$home" CAST_COLS=130 CAST_ROWS=60 \
  python3 ../../hub/tools/castrec.py asciiverse/settl.cast . "w70;kq;w1" \
  -- ./target/release/settl --demo
