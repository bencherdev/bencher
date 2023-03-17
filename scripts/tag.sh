#!/bin/bash

# Generate the Bencher CLI GitHub Action
cd ./services/action
npm install --include=dev
npm run build
cd -

# Generate the API docs from the server
cd ./services/api
cargo run --features swagger
cd -

git add Cargo.toml
git add Cargo.lock
git add ./services/action/dist/index.js
# If there was a change/the git tree is dirty add the updated file and commit
SWAGGER=./services/ui/src/components/docs/api/swagger.json
git diff --quiet $SWAGGER || git add $SWAGGER

TAG="$(./scripts/v.sh)"
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push
git push origin $TAG
