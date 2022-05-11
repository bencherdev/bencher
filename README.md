# Bencher 

rustup target add wasm32-unknown-unknown

cargo install --locked cargo-watch
cargo install --locked cargo-edit --features vendored-openssl
cargo install --locked cargo-udeps --features vendored-openssl
cargo install --locked cargo-audit --features vendored-openssl

cargo install --locked trunk

cargo install --locked wasm-pack

cd bencher
cargo run -- -x "cargo bench" --url http://localhost/v0/reports --email epompeii@protonmail.com --token 123JWT

cargo run --bin bencher -- ...

diesel setup

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

Repository: `bencher`
`gcloud artifacts repositories list`

Working Dir:
`cd api/demo`

Build Artifact:
`gcloud builds submit --timeout 1200 --tag us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest .`

Deploy to Cloud Run:
`gcloud run deploy fn-demo --image us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest`

Create Network Endpoint Groups:
`gcloud compute network-endpoint-groups create bencher-neg --region=us-central1 --network-endpoint-type=serverless --cloud-run-service=fn-demo`

Old Container Registry way:
`gcloud auth configure-docker`
`docker tag fn-demo us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo`
`docker push us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo`

NEW PROJECT:
`bencher` instead of `learned-stone-349519`
Setup `api.bencher.dev` to hit the load balancer.
This will make things much simpler for DNS purposes and separate hosting of the front and backend.