#!/bin/bash

# Generate the API docs from the server
cd ./services/api
cargo run --features swagger
cd -

# If there was a change/the git tree is dirty add the updated file and commit
SWAGGER=./services/ui/src/components/docs/api/swagger.json
git diff --quiet $SWAGGER || git add $SWAGGER

TAG=$(./scripts/version.sh)
COMMIT="Release $TAG"
echo $COMMIT
git commit -m "$COMMIT"
git tag $TAG
git push origin $TAG
