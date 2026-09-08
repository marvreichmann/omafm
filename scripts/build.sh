#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $(uname -m) == x86_64 ]] || { echo 'This release targets x86-64 Linux.' >&2; exit 1; }
cargo build --release --locked
mkdir -p bin
install -m755 target/release/omasdr bin/omasdr
sha256sum bin/omasdr > bin/SHA256SUMS
python3 scripts/release.py
omarchy plugin validate dist/marv.omasdr
printf 'Built dist/marv.omasdr and release archive.\n'
