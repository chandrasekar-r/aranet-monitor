#!/bin/sh
# Builds and ad-hoc signs target/AranetBar.app. Source from bundle.sh or package-release.sh.
set -eu

build_app_bundle() {
	./scripts/fetch_sparkle.sh

	if [ -n "${SPARKLE_EDDSA_PUBLIC_KEY:-}" ]; then
		/usr/libexec/PlistBuddy -c "Set :SUPublicEDKey $SPARKLE_EDDSA_PUBLIC_KEY" macos/Info.plist
	fi

	cargo build --release --features macos-app
	app=target/AranetBar.app
	rm -rf "$app"
	mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks"
	cp macos/Info.plist "$app/Contents/"
	cp target/release/aranetbar "$app/Contents/MacOS/"

	if [ -d macos/Frameworks/Sparkle.framework ]; then
		cp -R macos/Frameworks/Sparkle.framework "$app/Contents/Frameworks/"
		install_name_tool -add_rpath "@executable_path/../Frameworks" "$app/Contents/MacOS/aranetbar" 2>/dev/null || true
	fi

	codesign --force --deep --sign - --identifier com.20deg.aranetbar "$app"
}
