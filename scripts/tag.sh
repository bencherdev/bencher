#!/bin/bash

VERSION=$(cargo bin-version)

git add Cargo.toml
git add Cargo.lock
git add ./services/console/src/chunks/docs-reference/changelog/en/changelog.mdx

sed -i '' "s/version: [0-9]*\.[0-9]*\.[0-9]*/version: $VERSION/" ./README.md
git add ./README.md

# Generate the API docs from the server and the types for the UI
cargo gen-types
git add ./services/api/openapi.json
git add ./services/console/src/types/bencher.ts

# Generate CLI installer scripts
cargo gen-installer
git add ./services/cli/templates/output/install-cli.sh
git add ./services/cli/templates/output/install-cli.ps1

# Generate the Bencher CLI GitHub Action
cd ./services/action
npm install --include=dev
npm run build
git add ./dist/index.js
cd -

# Update CI workflow version
sed -i '' "s/bencher-version: \"[0-9]*\.[0-9]*\.[0-9]*\"/bencher-version: \"$VERSION\"/" ./.github/workflows/ci.yml
git add ./.github/workflows/ci.yml
# Update UI version and types
cd ./services/console
npm version $VERSION
git add ./package.json
git add ./package-lock.json
sed -i '' "s/version: [0-9]*\.[0-9]*\.[0-9]*/version: $VERSION/" ./src/chunks/docs-how-to/install-cli/cli-github-actions-version.mdx
git add ./src/chunks/docs-how-to/install-cli/cli-github-actions-version.mdx
sed -i '' "s/Bencher API Server v[0-9]*\.[0-9]*\.[0-9]*/Bencher API Server v$VERSION/" ./src/chunks/docs-tutorial/docker/bencher-up-output.mdx
git add ./src/chunks/docs-tutorial/docker/bencher-up-output.mdx
sed -i '' "s/bencher [0-9]*\.[0-9]*\.[0-9]*/bencher $VERSION/" ./src/chunks/docs-tutorial/quick-start/bencher-version-output.mdx
git add ./src/chunks/docs-tutorial/quick-start/bencher-version-output.mdx
sed -i '' "s/export BENCHER_VERSION=[0-9]*\.[0-9]*\.[0-9]*/export BENCHER_VERSION=$VERSION/" ./src/chunks/general/cli-unix-script-version.mdx
git add ./src/chunks/general/cli-unix-script-version.mdx
sed -i '' "s/\$env:BENCHER_VERSION=\"[0-9]*\.[0-9]*\.[0-9]*\"/\$env:BENCHER_VERSION=\"$VERSION\"/" ./src/chunks/general/cli-windows-script-version.mdx
git add ./src/chunks/general/cli-windows-script-version.mdx
sed -i '' "s/export BENCHER_VERSION=[0-9]*\.[0-9]*\.[0-9]*/export BENCHER_VERSION=$VERSION/" ./src/content/onboard/en/install-cli-version.mdx
git add ./src/content/onboard/en/install-cli-version.mdx
sed -i '' "s/\$env:BENCHER_VERSION=\"[0-9]*\.[0-9]*\.[0-9]*\"/\$env:BENCHER_VERSION=\"$VERSION\"/" ./src/content/onboard/en/install-cli-version.mdx
git add ./src/content/onboard/en/install-cli-version.mdx
cd -

TAG="v$VERSION"
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push
git push origin $TAG
