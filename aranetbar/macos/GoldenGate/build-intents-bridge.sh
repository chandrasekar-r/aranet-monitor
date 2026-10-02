#!/bin/sh
# Optional: link App Intents bridge static library into AranetBar.app (macOS + Xcode CLT Swift).
set -eu
app="${1:-../../target/AranetBar.app}"
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here"

if ! command -v swift >/dev/null 2>&1; then
	echo "swift not found; skipping intents bridge"
	exit 0
fi

swift build -c release 2>/dev/null || {
	echo "swift build failed; skipping intents bridge"
	exit 0
}

lib=".build/release/libAranetBarIntentsBridge.a"
bin="$app/Contents/MacOS/aranetbar"
if [ ! -f "$lib" ] || [ ! -f "$bin" ]; then
	exit 0
fi

# Re-link binary with bridge (best-effort dev helper).
tmp="${bin}.linked"
if clang "$bin" "$lib" -framework AppIntents -framework Foundation -o "$tmp" 2>/dev/null; then
	mv "$tmp" "$bin"
	echo "Linked App Intents bridge into $bin"
fi
