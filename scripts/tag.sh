#!/bin/bash

VERSION=$(./scripts/version.sh)

git add Cargo.toml
git add Cargo.lock
git add ./services/console/src/chunks/reference/en/changelog.mdx

# Generate the API docs from the server and the types for the UI
cargo xtask types
git add ./services/api/swagger.json
git add ./services/console/src/types/bencher.ts

# Generate the Bencher CLI GitHub Action
cd ./services/action
npm install --include=dev
npm run build
git add ./dist/index.js
cd -

# Update UI version and types
cd ./services/console
npm version $VERSION
git add ./package.json
git add ./package-lock.json
cd -

TAG="v$VERSION"
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push
git push origin $TAG
