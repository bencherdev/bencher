# Bencher

rustup target add wasm32-unknown-unknown

cargo install --locked cargo-watch
cargo install --locked cargo-edit --features vendored-openssl
cargo install --locked cargo-udeps --features vendored-openssl
cargo install --locked cargo-audit --features vendored-openssl

cargo install --locked trunk

cargo install --locked wasm-pack

cd bencher
cargo run -- run --local --adapter rust "cargo bench"
or
cargo bench | cargo run -- run --local --adapter rust

cargo run -- auth signup --host http://localhost:8080 --name "Gwenith Paltrow" goop@goop.com

cargo run -- auth login --host http://localhost:8080 goop@goop.com

cargo run -- project ls --host http://localhost:8080

cargo run -- project create --host http://localhost:8080 "Hazel River"

cargo run -- project view --host http://localhost:8080 hazel-river

cargo run -- testbed ls --host http://localhost:8080 --project hazel-river

cargo run -- testbed create --host http://localhost:8080 --project hazel-river --os-name macos --ram 32GB nemo

cargo run -- testbed view --host http://localhost:8080 --project hazel-river nemo



----

cargo run -- run --host http://localhost:8080 --adapter rust "cargo bench"

cargo run -- testbed create --host http://localhost:8080 --os-name macos --ram 32GB nemo






cargo run --bin bencher -- ...

cargo install diesel_cli --no-default-features --features postgres
diesel migration generate $MIGRATION_NAME
diesel setup
diesel migration run
diesel migration revert
diesel migration redo

cd ui
wasm-pack build

cd www
npm run start

Dev Setup:
`docker compose -f docker-compose.yml up -d --build --remove-orphans`

If UI deps added:
`docker compose -f docker-compose.yml build --no-cache`

Region: `us-central1`
`gcloud config set run/region us-central1`

Project: `learned-stone-349519`
`gcloud config get-value project`

`gcloud config set project bencher`
`gcloud config get-value project`
`bencher`

Repository: `bencher`
`gcloud artifacts repositories list`

Working Dir:
`cd api/demo`

Build Artifact:
`gcloud builds submit --timeout 1200 --tag us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest .`

`gcloud builds submit --timeout 1200 --tag us-central1-docker.pkg.dev/bencher/bencher/fn-demo:latest .`


`gcloud builds submit --timeout 1200 --tag us-central1-docker.pkg.dev/bencher/bencher/fn-demo:latest .`

Deploy to Cloud Run:
`gcloud run deploy fn-demo --image us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest`

`gcloud run deploy fn-demo --image us-central1-docker.pkg.dev/bencher/bencher/fn-demo:latest`

Create Network Endpoint Groups:
`gcloud compute network-endpoint-groups create bencher-neg --region=us-central1 --network-endpoint-type=serverless --cloud-run-service=fn-demo`

`gcloud compute network-endpoint-groups create bencher --region=us-central1 --network-endpoint-type=serverless --cloud-run-service=fn-demo`


`cloud_sql_proxy`
`./cloud_sql_proxy -dir ./Code/db `


Run from repo root:
Note that the `--ignore-file` path is relative to the context `./services`

`gcloud builds submit --timeout 1800 --config ./services/api/swagger/cloudbuild.yaml --ignore-file ./api/swagger/.gcloudignore ./services` 

`gcloud builds submit --timeout 1800 --config ./services/api/admin/cloudbuild.yaml --ignore-file ./api/admin/.gcloudignore ./services` 

`gcloud builds submit --timeout 1800 --config ./services/api/reports/cloudbuild.yaml --ignore-file ./api/reports/.gcloudignore ./services` 

---

`gcloud run deploy fn-swagger --image us-central1-docker.pkg.dev/bencher/bencher/fn-swagger:latest --allow-unauthenticated`

`gcloud run deploy fn-admin --image us-central1-docker.pkg.dev/bencher/bencher/fn-admin:latest --allow-unauthenticated --add-cloudsql-instances INSTANCE_CONNECTION_NAME --update-secrets BENCHER_DB_URL=SECRET/RESOURCE/ID:latest`

`gcloud run deploy fn-reports --image us-central1-docker.pkg.dev/bencher/bencher/fn-reports:latest --allow-unauthenticated --add-cloudsql-instances INSTANCE_CONNECTION_NAME --update-secrets BENCHER_DB_URL=SECRET/RESOURCE/ID:latest`

---

`gcloud compute network-endpoint-groups list`

`gcloud compute network-endpoint-groups create fn-swagger --network-endpoint-type=serverless --region=us-central1 --cloud-run-service=fn-swagger`

`gcloud compute network-endpoint-groups create fn-admin --network-endpoint-type=serverless --region=us-central1 --cloud-run-service=fn-admin`

`gcloud compute network-endpoint-groups create fn-reports --network-endpoint-type=serverless --region=us-central1 --cloud-run-service=fn-reports`

https://cloud.google.com/sdk/gcloud/reference/compute/backend-services

`gcloud compute backend-services describe bencher --global`

`gcloud compute backend-services create fn-swagger --global --load-balancing-scheme EXTERNAL_MANAGED`
`gcloud compute backend-services describe fn-swagger --global`
`gcloud compute backend-services add-backend fn-swagger --global --network-endpoint-group-region us-central1 --network-endpoint-group fn-swagger`

`gcloud compute backend-services create fn-admin --global --load-balancing-scheme EXTERNAL_MANAGED`
`gcloud compute backend-services describe fn-admin --global`
`gcloud compute backend-services add-backend fn-admin --global --network-endpoint-group-region us-central1 --network-endpoint-group fn-admin`

`gcloud compute backend-services create fn-reports --global --load-balancing-scheme EXTERNAL_MANAGED`
`gcloud compute backend-services describe fn-reports --global`
`gcloud compute backend-services add-backend fn-reports --global --network-endpoint-group-region us-central1 --network-endpoint-group fn-reports`

`gcloud compute backend-buckets create docs --gcs-bucket-name docs.bencher.dev`
`gcloud compute backend-buckets describe docs`

add-backend | update-backend | remove-backend

`gcloud compute url-maps list`

`gcloud compute url-maps describe bencher`

`gcloud compute url-maps export bencher --destination ./url-map.yaml --global`

`gcloud compute url-maps validate --source ./url-map.yaml --load-balancing-scheme EXTERNAL_MANAGED`

`gcloud compute url-maps import bencher --source ./url-map.yaml --global --quiet`


Connect via port `3307`

Update CLI:
`gcloud components update`

`gcloud compute forwarding-rules list`

`gcloud compute backend-services list`

`gcloud compute network-endpoint-groups list`




Old Container Registry way:
`gcloud auth configure-docker`
`docker tag fn-demo us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo`
`docker push us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo`

NEW PROJECT:
`bencher` instead of `learned-stone-349519`
Setup `api.bencher.dev` to hit the load balancer.
This will make things much simpler for DNS purposes and separate hosting of the front and backend.

https://github.com/ryansolid/solid-ts-webpack
npm install
{
  "scripts": {
    "dev": "vite", // start dev server, aliases: `vite dev`, `vite serve`
    "build": "vite build", // build for production
    "preview": "vite preview" // locally preview production build
  }
}

npx prettier --write .
npx prettier --check .

docker compose --file docs.docker-compose.yml up --build -d 

gcloud sql connect bencher --project=bencher --user=postgres --quiet

https://cloud.google.com/sql/docs/postgres/connect-run
https://stackoverflow.com/questions/60544602/postgrest-on-google-cloud-sql-unix-socket-uri-format

Create secret:

This works:
postgres://<pg_user>:<pg_pass>@/<db_name>?host=/cloudsql/<cloud_sql_instance_connection_name>

May be necessary for some frameworks (/.s.PGSQL.5432):
postgres://<pg_user>:<pg_pass>@/<db_name>?host=/cloudsql/<cloud_sql_instance_connection_name>/.s.PGSQL.5432

gcloud secrets create BENCHER_DB_URL --replication-policy="automatic"

echo -n "my_secret" | gcloud secrets versions add BENCHER_DB_URL --data-file=-

gcloud secrets describe BENCHER_DB_URL

gcloud secrets versions access 1 --secret=BENCHER_DB_URL
gcloud secrets versions access latest --secret=BENCHER_DB_URL

Cloud Run GUI -> Variables & Secrets -> Secrets -> Add secret as environment variable BENCHER_DB_URL as latest

gcloud sql instances describe bencher

Cloud Run GUI -> Connections -> Cloud SQL Connections -> Add db as a connection

Hot Reloading 

https://github.com/vitejs/vite/issues/4116
https://github.com/jonathan-f-silva/vite-docker-hmr-dev-base

Migrate:

https://api.bencher.dev/v0/admin/migrate

Docs:
https://dev.solidjs.com/guides/server#getting-started-with-static-site-generation
https://github.com/solidjs/solid/issues/477
https://github.com/olgam4/bat

`mdbook serve --open`

https://cloud.google.com/storage/docs/hosting-static-website

`gsutil cp -r ./services/docs/book/* gs://docs.bencher.dev/`
`gsutil iam ch allUsers:objectViewer gs://docs.bencher.dev/`
`gsutil web set -m index.html gs://docs.bencher.dev/`

Auth - ORY
https://www.ory.sh/docs/guides/cli/installation
https://www.ory.sh/docs/kratos/concepts/ui-user-interface
https://www.ory.sh/docs/guides/social-signin/github

Storybook
npm x start-storybook
npm run start

Litestream
cd services/api
export LITESTREAM_ACCESS_KEY_ID=***
export LITESTREAM_SECRET_ACCESS_KEY=***
export LITESTREAM_DB_PATH=bencher.db
export LITESTREAM_REPLICA_URL=s3://db.bencher.dev/bencher.db

litestream restore -o $LITESTREAM_DB_PATH $LITESTREAM_REPLICA_URL
litestream replicate $LITESTREAM_DB_PATH $LITESTREAM_REPLICA_URL

litestream replicate --config ./litestream.yml

docker build -f Dockerfile --tag api-lite ..

docker run -p 8080:8080 -e LITESTREAM_ACCESS_KEY_ID=$LITESTREAM_ACCESS_KEY_ID -e LITESTREAM_SECRET_ACCESS_KEY=$LITESTREAM_SECRET_ACCESS_KEY -e LITESTREAM_DB_PATH=$LITESTREAM_DB_PATH -e LITESTREAM_REPLICA_URL=$LITESTREAM_REPLICA_URL --name api_lite --rm api-lite

# WASM
wasm-pack build . --target web --features wasm
