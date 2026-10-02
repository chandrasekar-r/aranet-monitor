#!/bin/sh
# Builds AranetBar.app, ad-hoc signs it and installs it to ~/Applications.
set -eu
cd "$(dirname "$0")"
. ./build-app.sh

build_app_bundle

dest="$HOME/Applications/AranetBar.app"
mkdir -p "$HOME/Applications"
pkill -x aranetbar 2>/dev/null || true
rm -rf "$dest"
cp -R target/AranetBar.app "$dest"
echo "Installed $dest"
