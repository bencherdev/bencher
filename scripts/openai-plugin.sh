#!/bin/bash
set -euo pipefail

# Build the OpenAI plugin ZIP: [OPENAI_DEMO_RECORDING_URL=https://...] ./scripts/openai-plugin.sh [OUT_DIR]
OUT_DIR="${1:-$(mktemp -d)}"
mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

cd "$(dirname "$0")/../plugins/bencher"
VERSION=$(sed -n 's/^  "version": "\([0-9]*\.[0-9]*\.[0-9]*\)",$/\1/p' plugin.json)
: "${VERSION:?no version in plugin.json}"
ZIP="$OUT_DIR/bencher-openai-plugin-$VERSION.zip"
rm -f "$ZIP"

if [ -z "${OPENAI_DEMO_RECORDING_URL:-}" ]; then
    echo "warning: OPENAI_DEMO_RECORDING_URL is unset, so the ZIP has no demo_recording_url" >&2
    cp plugin.json "$STAGE/plugin.json"
else
    python3 -I -c '
import json, sys
url, src, dst = sys.argv[1:]
if not url.startswith("https://"):
    sys.exit("OPENAI_DEMO_RECORDING_URL must start with https://")
text = open(src, encoding="utf-8", newline="").read()
anchor = "      \"review\": {\n"
staged = text.replace(anchor, anchor + "        \"demo_recording_url\": " + json.dumps(url) + ",\n", 1)
expected = json.loads(text)
expected["extensions"]["com.openai"]["review"]["demo_recording_url"] = url
if staged == text or json.loads(staged) != expected:
    sys.exit("could not add demo_recording_url to plugin.json")
open(dst, "w", encoding="utf-8", newline="").write(staged)
' "$OPENAI_DEMO_RECORDING_URL" plugin.json "$STAGE/plugin.json"
fi

zip -q -r -X "$ZIP" mcp.json skills assets -x '.*' '*/.*'
(cd "$STAGE" && zip -q -X "$ZIP" plugin.json)
echo "$ZIP"
