#!/bin/bash
# Build a distributable DMG from a signed .app payload, on a macOS machine.
#
#   packaging/macos/dmg.sh PAYLOAD OUTPUT.dmg
#
# PAYLOAD is a directory whose only contents are the signed
# "Atlas Commerce Desktop.app" bundle; the DMG adds an /Applications symlink.
# native-packages supplies an owned, already-signed copy when Apple
# credentials are configured, so this script only packages.
set -euo pipefail

payload="$1"
output="$2"

[ -d "$payload" ] || { echo "payload directory not found: $payload" >&2; exit 1; }
[ ! -e "$output" ] || { echo "output already exists: $output" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cp -R "$payload/." "$tmp/"
app="$tmp/Atlas Commerce Desktop.app"
[ -d "$app" ] || { echo "expected the signed Atlas Commerce Desktop.app input" >&2; exit 1; }
ln -s /Applications "$tmp/Applications"

hdiutil create -volname "Atlas Commerce Desktop" -srcfolder "$tmp" -format UDZO "$output"
hdiutil verify "$output"
