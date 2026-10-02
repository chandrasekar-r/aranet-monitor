#!/usr/bin/env bash
# Downloads Sparkle 2.x into macos/Frameworks/Sparkle.framework (gitignored).
set -euo pipefail
cd "$(dirname "$0")/.."

SPARKLE_VERSION="${SPARKLE_VERSION:-2.7.0}"
dest="macos/Frameworks"
framework="$dest/Sparkle.framework"

if [[ -d "$framework" ]]; then
  echo "Sparkle already present at $framework"
  exit 0
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

url="https://github.com/sparkle-project/Sparkle/releases/download/${SPARKLE_VERSION}/Sparkle-${SPARKLE_VERSION}.tar.xz"
echo "Fetching $url"
curl -fsSL "$url" -o "$tmpdir/sparkle.tar.xz"
tar -xJf "$tmpdir/sparkle.tar.xz" -C "$tmpdir"

mkdir -p "$dest" "$dest/sparkle-tools"
if [[ -d "$tmpdir/Sparkle.framework" ]]; then
  cp -R "$tmpdir/Sparkle.framework" "$dest/"
elif [[ -d "$tmpdir/Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework" ]]; then
  cp -R "$tmpdir/Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework" "$dest/"
else
  echo "Unexpected Sparkle archive layout" >&2
  ls -laR "$tmpdir" >&2
  exit 1
fi

for tool in generate_appcast generate_keys sign_update; do
  if [[ -f "$tmpdir/bin/$tool" ]]; then
    install -m 755 "$tmpdir/bin/$tool" "$dest/sparkle-tools/$tool"
  fi
done

echo "Installed $framework"
