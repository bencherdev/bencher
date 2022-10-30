#!/bin/bash

# Generate the API docs from the server
cd ./services/api
cargo run --features swagger
cd -

# If there was a change/the git tree is dirty add the updated file and commit
git diff --quiet || (git add ./services/ui/src/components/docs/api/swagger.json && git commit -m "Update Swagger")

TAG=$(./scripts/version.sh)
git tag $TAG
git push origin $TAG
