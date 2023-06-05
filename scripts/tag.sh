#!/bin/bash

VERSION=$(./scripts/version.sh)

# Generate the API docs from the server
cd ./services/api
cargo run --features swagger
cd -

# Generate the Bencher CLI GitHub Action
cd ./services/action
npm install --include=dev
npm run build
cd -

# Update UI version and types
cd ./services/ui
npm version $VERSION
npm run typeshare
cd -

git add Cargo.toml
git add Cargo.lock
git add ./services/action/dist/index.js
git add ./services/ui/package.json
git add ./services/ui/package-lock.json
git add ./services/ui/src/components/docs/api/swagger.json
git add ./services/ui/src/components/docs/pages/reference/Changelog.mdx
git add ./services/ui/src/types/bencher.d.ts

TAG="v$VERSION"
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push
git push origin $TAG
