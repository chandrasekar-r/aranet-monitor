#!/bin/sh
# Builds and ad-hoc signs target/AranetBar.app. Source from bundle.sh or package-release.sh.
set -eu

build_app_bundle() {
	cargo build --release
	app=target/AranetBar.app
	rm -rf "$app"
	mkdir -p "$app/Contents/MacOS"
	cp macos/Info.plist "$app/Contents/"
	cp target/release/aranetbar "$app/Contents/MacOS/"
	codesign --force --sign - --identifier com.20deg.aranetbar "$app"
}
