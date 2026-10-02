#!/bin/sh
# Builds AranetBar.app, ad-hoc signs it and installs it to ~/Applications.
set -eu
cd "$(dirname "$0")"

cargo build --release
app=target/AranetBar.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp macos/Info.plist "$app/Contents/"
cp target/release/aranetbar "$app/Contents/MacOS/"
codesign --force --sign - --identifier com.20deg.aranetbar "$app"

dest="$HOME/Applications/AranetBar.app"
mkdir -p "$HOME/Applications"
pkill -x aranetbar 2>/dev/null || true
rm -rf "$dest"
cp -R "$app" "$dest"
echo "Installed $dest"
