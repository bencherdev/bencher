#!/bin/bash
set -euo pipefail

# Release a new version from devel: ./scripts/tag.sh <minor|patch>
BUMP="${1:-}"
CHANGELOG=./services/console/src/chunks/docs-reference/changelog/en/changelog.mdx

fail() {
    echo "$1" >&2
    exit 1
}

# In-place edit that works with both GNU and BSD sed
sedi() {
    sed -i.bak "$1" "$2"
    rm "$2.bak"
}

if [ $# -ne 1 ] || { [ "$BUMP" != "minor" ] && [ "$BUMP" != "patch" ]; }; then
    fail "Usage: $0 <minor|patch>"
fi
if [ "$(git branch --show-current)" != "devel" ]; then
    fail "Not on devel"
fi
if [ -n "$(git status --porcelain)" ]; then
    fail "Working tree is not clean"
fi
git fetch origin devel --tags
if [ "$(git rev-parse devel)" != "$(git rev-parse origin/devel)" ]; then
    fail "devel is not origin/devel"
fi

CURRENT=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)
if ! [[ "$CURRENT" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    fail "No single workspace version in Cargo.toml"
fi
if [ "$BUMP" = "minor" ]; then
    VERSION="${BASH_REMATCH[1]}.$((BASH_REMATCH[2] + 1)).0"
else
    VERSION="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.$((BASH_REMATCH[3] + 1))"
fi
TAG="v$VERSION"

if git rev-parse -q --verify "refs/tags/$TAG" > /dev/null || git ls-remote --exit-code --tags origin "refs/tags/$TAG" > /dev/null; then
    fail "$TAG already exists"
fi
if [[ "$(head -n 1 "$CHANGELOG")" != "## Pending "* ]]; then
    fail "The changelog does not start with a Pending header"
fi

sedi "s/^version = \"$CURRENT\"$/version = \"$VERSION\"/" Cargo.toml
git add Cargo.toml
cargo update --workspace
git add Cargo.lock
if [ "$(cargo bin-version)" != "$VERSION" ]; then
    fail "cargo bin-version does not print $VERSION"
fi

sedi "1s/.*/## \`$TAG\`/" "$CHANGELOG"
git add "$CHANGELOG"

sedi "s/version: [0-9]*\.[0-9]*\.[0-9]*/version: $VERSION/" ./README.md
git add ./README.md

sedi "s/\"version\": \"[0-9]*\.[0-9]*\.[0-9]*\"/\"version\": \"$VERSION\"/" ./plugins/bencher/.claude-plugin/plugin.json
git add ./plugins/bencher/.claude-plugin/plugin.json
sedi "s/\"version\": \"[0-9]*\.[0-9]*\.[0-9]*\"/\"version\": \"$VERSION\"/" ./plugins/bencher/plugin.json
git add ./plugins/bencher/plugin.json

# Generate the API docs from the server and the types for the UI
cargo gen-types
git add ./services/api/openapi.json
git add ./services/console/src/types/bencher.ts

# Generate installer scripts
cargo gen-installer
git add ./services/cli/templates/output/install-cli.sh
git add ./services/cli/templates/output/install-cli.ps1
git add ./services/runner/templates/output/install-runner.sh

# Generate the Bencher CLI GitHub Action
(
    cd ./services/action
    npm install --include=dev
    npm run build
    git add ./dist/index.js
)

# Update UI version and docs
(
    cd ./services/console
    npm version "$VERSION"
    git add ./package.json
    git add ./package-lock.json
    sedi "s/version: [0-9]*\.[0-9]*\.[0-9]*/version: $VERSION/" ./src/chunks/docs-how-to/install-cli/cli-github-actions-version.mdx
    git add ./src/chunks/docs-how-to/install-cli/cli-github-actions-version.mdx
    sedi "s|bencher:v[0-9]*\.[0-9]*\.[0-9]*|bencher:v$VERSION|" ./src/chunks/docs-how-to/install-cli/cli-docker-version.mdx
    git add ./src/chunks/docs-how-to/install-cli/cli-docker-version.mdx
    sedi "s/Bencher API Server v[0-9]*\.[0-9]*\.[0-9]*/Bencher API Server v$VERSION/" ./src/chunks/docs-tutorial/self-hosted/bencher-up-output.mdx
    git add ./src/chunks/docs-tutorial/self-hosted/bencher-up-output.mdx
    sedi "s/bencher [0-9]*\.[0-9]*\.[0-9]*/bencher $VERSION/" ./src/chunks/docs-tutorial/quickstart/bencher-version-output.mdx
    git add ./src/chunks/docs-tutorial/quickstart/bencher-version-output.mdx
    sedi "s/export BENCHER_VERSION=[0-9]*\.[0-9]*\.[0-9]*/export BENCHER_VERSION=$VERSION/" ./src/chunks/general/cli-unix-script-version.mdx
    git add ./src/chunks/general/cli-unix-script-version.mdx
    sedi "s/\$env:BENCHER_VERSION=\"[0-9]*\.[0-9]*\.[0-9]*\"/\$env:BENCHER_VERSION=\"$VERSION\"/" ./src/chunks/general/cli-windows-script-version.mdx
    git add ./src/chunks/general/cli-windows-script-version.mdx
    sedi "s/export BENCHER_VERSION=[0-9]*\.[0-9]*\.[0-9]*/export BENCHER_VERSION=$VERSION/" ./src/content/onboard/en/install-cli-version.mdx
    sedi "s/\$env:BENCHER_VERSION=\"[0-9]*\.[0-9]*\.[0-9]*\"/\$env:BENCHER_VERSION=\"$VERSION\"/" ./src/content/onboard/en/install-cli-version.mdx
    git add ./src/content/onboard/en/install-cli-version.mdx
)

git commit -m "Release $TAG"
git tag "$TAG"
# Both refs move or neither does, so a failed run never leaves devel with an untagged release
git push --atomic origin devel "$TAG"
