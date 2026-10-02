#!/bin/sh
# Builds AranetBar.app plus AranetBar-vVERSION.dmg and .zip under target/ (no install).
set -eu
cd "$(dirname "$0")"
. ./build-app.sh

VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

build_app_bundle

dmg="target/AranetBar-v${VERSION}.dmg"
zip="target/AranetBar-v${VERSION}.zip"
rm -f "$dmg" "$zip"

hdiutil create -volname "AranetBar" -srcfolder target/AranetBar.app -ov -format UDZO "$dmg"
ditto -c -k --keepParent target/AranetBar.app "$zip"

echo "Built $dmg ($(du -h "$dmg" | cut -f1))"
echo "Built $zip ($(du -h "$zip" | cut -f1))"
