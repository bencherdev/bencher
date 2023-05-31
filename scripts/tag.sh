#!/bin/bash

# Generate the API docs from the server
cd ./services/api
cargo run --features swagger
cd -

VERSION=$(./scripts/version.sh)

# Generate the Bencher CLI GitHub Action
cd ./services/action
npm install --include=dev
npm run build
cd -

# Update UI version
cd ./services/ui
npm version $VERSION
cd -

git add Cargo.toml
git add Cargo.lock
git add ./services/action/dist/index.js
git add ./services/ui/package.json
git add ./services/ui/package-lock.json
# If there was a change/the git tree is dirty add the updated file and commit
SWAGGER=./services/ui/src/components/docs/api/swagger.json
git diff --quiet $SWAGGER || git add $SWAGGER

TAG="v$VERSION"
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push
git push origin $TAG
