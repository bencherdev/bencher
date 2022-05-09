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

docker run --rm -it -p 9901:9901 -p 10000:10000  -v $(pwd)/envoy/config.yml:/config.yml envoyproxy/envoy:v1.21.2 -c /config.yml

docker-compose -f docker-compose.yml up -d --build --remove-orphans

gcloud run deploy --source . --project learned-stone-349519
gcloud run deploy --image fn-demo --project learned-stone-349519
gcloud config set run/region us-central1

gcloud builds submit --tag [IMAGE] .
gcloud run deploy fn-demo --image [IMAGE]

gcloud artifacts repositories list
cloud-run-source-deploy
gcloud config set project learned-stone-349519
gcloud config get-value project

gcloud builds submit --tag us-central1-docker.pkg.dev/learned-stone-349519/cloud-run-source-deploy/fn-demo:latest

us.gcr.io/learned-stone-349519/fn-demo
docker tag fn-demo us.gcr.io/learned-stone-349519/fn-demo
gcloud auth configure-docker
docker push us.gcr.io/learned-stone-349519/fn-demo

Container Registry is still serving *gcr.io traffic. Copy your images from us.gcr.io/learned-stone-349519 to us-docker.pkg.dev/learned-stone-349519/us.gcr.io, then route traffic to Artifact Registry to continue the transition. Learn more

us.gcr.io/learned-stone-349519
us-docker.pkg.dev/us.gcr.io/learned-stone-349519
docker tag fn-demo us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo
gcloud auth configure-docker
docker push us-docker.pkg.dev/us.gcr.io/learned-stone-349519/fn-demo

gcloud builds submit --tag us-central1-docker.pkg.dev/earned-stone-349519/quickstart-docker-repo/quickstart-image:tag1

gcloud artifacts repositories list
gcloud config get-value project

docker tag fn-demo us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo
gcloud builds submit --tag us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo
gcloud builds submit --region=us-west2 --tag us-west2-docker.pkg.dev/project-id/quickstart-docker-repo/quickstart-image:tag1
gcloud builds submit --region=us-central1 --source . --tag us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest



gcloud builds submit --timeout 1200 --tag us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest .
gcloud run deploy fn-demo --image us-central1-docker.pkg.dev/learned-stone-349519/bencher/fn-demo:latest
