#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Quickshell supplies the qs.* import prefix at runtime. Give qmllint the same
# prefix outside the plugin; installed plugin folders may not contain symlinks.
imports=$(mktemp -d /tmp/omasdr-lint.XXXXXX)
trap 'rm -rf -- "$imports"' EXIT
ln -s "${OMARCHY_PATH:-/usr/share/omarchy}/shell" "$imports/qs"
lint=$(command -v qmllint || printf /usr/lib/qt6/bin/qmllint)
"$lint" -I "$imports" BarWidget.qml Panel.qml Receiver.qml Bookmarks.qml
