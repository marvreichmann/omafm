#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $(uname -m) == x86_64 ]] || { echo 'This release targets x86-64 Linux.' >&2; exit 1; }
cargo build --release --locked
mkdir -p bin
install -m755 target/release/omafm bin/omafm
sha256sum bin/omafm > bin/SHA256SUMS
python3 scripts/release.py
# release.py names the packaged folder after the manifest id; read it back
# rather than repeating the id here, so a rename only happens in one place.
plugin=$(jq -r .id manifest.json)
omarchy plugin validate "dist/$plugin"
printf 'Built dist/%s and release archive.\n' "$plugin"
