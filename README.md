# Bencher 

rustup target add wasm32-unknown-unknown

cargo install --locked cargo-watch
cargo install --locked cargo-edit --features vendored-openssl
cargo install --locked cargo-udeps --features vendored-openssl
cargo install --locked cargo-audit --features vendored-openssl

cargo install --locked trunk

cargo install --locked wasm-pack

cd bencher
cargo run -- -x "cargo bench" --url http://localhost:8080/ repo --url git@github.com:epompeii/bencher_db.git --key $HOME/.ssh/id_ed25519 --push

cargo run --bin bencher -- ...

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
`cd functions/demo`

Build Artifact:
`gcloud builds submit --timeout 1200 --tag us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest .`

Deploy to Cloud Run:
`gcloud run deploy fn-demo --image us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest`

Old Container Registry way:
`gcloud auth configure-docker`
`docker tag fn-demo us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo`
`docker push us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo`
