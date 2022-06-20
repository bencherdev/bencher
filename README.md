# Bencher 

rustup target add wasm32-unknown-unknown

cargo install --locked cargo-watch
cargo install --locked cargo-edit --features vendored-openssl
cargo install --locked cargo-udeps --features vendored-openssl
cargo install --locked cargo-audit --features vendored-openssl

cargo install --locked trunk

cargo install --locked wasm-pack

cd bencher
cargo run -- --email epompeii@protonmail.com --token 123JWT --url http://localhost run --adapter rust "cargo bench" 

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

---

`gcloud run deploy fn-swagger --image us-central1-docker.pkg.dev/bencher/bencher/fn-swagger:latest --allow-unauthenticated`

`gcloud run deploy fn-admin --image us-central1-docker.pkg.dev/bencher/bencher/fn-admin:latest --allow-unauthenticated`

---

`gcloud compute network-endpoint-groups list`

`gcloud compute network-endpoint-groups create fn-swagger --network-endpoint-type=serverless --region=us-central1 --cloud-run-service=fn-swagger`

`gcloud compute network-endpoint-groups create fn-admin --network-endpoint-type=serverless --region=us-central1 --cloud-run-service=fn-admin`

https://cloud.google.com/sdk/gcloud/reference/compute/backend-services

`gcloud compute backend-services describe bencher --global`

`gcloud compute backend-services create fn-swagger --global --load-balancing-scheme EXTERNAL_MANAGED`
`gcloud compute backend-services describe fn-swagger --global`
`gcloud compute backend-services add-backend fn-swagger --global --network-endpoint-group-region us-central1 --network-endpoint-group fn-swagger`

`gcloud compute backend-services create fn-admin --global --load-balancing-scheme EXTERNAL_MANAGED`
`gcloud compute backend-services describe fn-admin --global`
`gcloud compute backend-services add-backend fn-admin --global --network-endpoint-group-region us-central1 --network-endpoint-group fn-admin`

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

mdbook serve --open