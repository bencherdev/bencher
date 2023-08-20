#!/bin/bash

URL=${1:-https://bencher.dev}
VERSION=$(./scripts/version.sh)

if curl -s "$URL" | grep -q "<div class=\"navbar-item\">BETA v<!--#-->$VERSION<!--/--></div>"; then
    echo "Console UI up to date: $VERSION"
    exit 0
else
    echo "Console UI is not at version $VERSION."
    exit 1
fi