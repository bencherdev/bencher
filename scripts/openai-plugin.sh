#!/bin/bash
set -euo pipefail

# Build the OpenAI plugin ZIP: ./scripts/openai-plugin.sh [OUT_DIR]
OUT_DIR="${1:-$(mktemp -d)}"
mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)

cd "$(dirname "$0")/../plugins/bencher"
VERSION=$(sed -n 's/^  "version": "\([0-9]*\.[0-9]*\.[0-9]*\)",$/\1/p' plugin.json)
: "${VERSION:?no version in plugin.json}"
ZIP="$OUT_DIR/bencher-openai-plugin-$VERSION.zip"

rm -f "$ZIP"
zip -q -r -X "$ZIP" plugin.json mcp.json skills assets -x '.*' '*/.*'
echo "$ZIP"
