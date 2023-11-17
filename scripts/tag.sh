#!/bin/bash

VERSION=$(./scripts/version.sh)

# Generate the API docs from the server
cd ./services/api
cargo run --bin swagger --features swagger
cd -

# Generate the Bencher CLI GitHub Action
cd ./services/action
npm install --include=dev
npm run build
cd -

# Update UI version and types
cd ./services/console
npm version $VERSION
npm run typeshare
cd -

git add Cargo.toml
git add Cargo.lock
git add ./lib/bencher_valid/swagger.json
git add ./services/action/dist/index.js
git add ./services/console/package.json
git add ./services/console/package-lock.json
git add ./services/console/src/content/reference/changelog.mdx
git add ./services/console/src/types/bencher.ts

TAG="v$VERSION"
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push
git push origin $TAG
