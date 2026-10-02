#!/usr/bin/env bash
# Generates a signed appcast.xml from AranetBar release .zip archives.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <directory-with-AranetBar-*.zip>" >&2
  exit 1
fi

archives_dir="$1"
tools_dir="${SPARKLE_TOOLS_DIR:-macos/Frameworks/sparkle-tools}"
generate="$tools_dir/generate_appcast"

if [[ ! -x "$generate" ]]; then
  echo "Run scripts/fetch_sparkle.sh first (or set SPARKLE_TOOLS_DIR)." >&2
  exit 1
fi

key_args=()
if [[ -n "${SPARKLE_EDDSA_PRIVATE_KEY:-}" ]]; then
  key_file="$(mktemp)"
  trap 'rm -f "$key_file"' EXIT
  printf '%s' "$SPARKLE_EDDSA_PRIVATE_KEY" >"$key_file"
  key_args=(--ed-key-file "$key_file")
elif [[ -f "${SPARKLE_EDDSA_PRIVATE_KEY_FILE:-}" ]]; then
  key_args=(--ed-key-file "$SPARKLE_EDDSA_PRIVATE_KEY_FILE")
elif [[ -f "$HOME/.sparkle_eddsa/private_key" ]]; then
  key_args=(--ed-key-file "$HOME/.sparkle_eddsa/private_key")
else
  echo "Set SPARKLE_EDDSA_PRIVATE_KEY or SPARKLE_EDDSA_PRIVATE_KEY_FILE." >&2
  exit 1
fi

"$generate" "${key_args[@]}" "$archives_dir"
cp "$archives_dir/appcast.xml" macos/appcast.xml
echo "Updated macos/appcast.xml"
