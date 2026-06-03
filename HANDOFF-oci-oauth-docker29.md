# Project-key OCI auth fails on Docker 29 (containerd image store) — repro + OAuth2 token fix

> Handoff doc for a local agent with real network access and a working Docker
> daemon. Goal: (1) reproduce the failure end-to-end and capture a **negative
> test**, (2) implement OAuth2 POST support on the OCI token endpoint, (3) turn
> the negative test green. Tested against `dev.registry.bencher.dev` (or fully
> local).

---

## 1. Context / what the user reported

Project-key OCI auth (`docker login registry.bencher.dev -u <project-slug> -p
bencher_run_…`, then `docker push`) **fails on `Docker version 29.5.2`** but
works unchanged with **podman**. The user asked us to at least document it.

Project keys on OCI endpoints are recent: added in `044781a` (PR #830); docs
started recommending them in `665fb66` (PR #848). CI covers the flow
(`tasks/test_api/src/task/plus/runner.rs:574-674` `run_project_key_runner_test`)
but on whatever Docker the CI host ships — not v29.

## 2. Root cause (verified against upstream source)

The token-fetch flow differs by client, and Docker 29 changed its default.

1. **Docker 29 makes the containerd image store the default** (new installs).
   Push/pull now go through containerd's registry-auth code, not the legacy
   moby path.
   - https://www.docker.com/blog/docker-engine-version-29/
   - https://docs.docker.com/engine/storage/containerd/
   - https://github.com/moby/moby/issues/51532

2. **containerd fetches the bearer token with an OAuth2 `POST` *first*, with
   credentials in the form body** (not a Basic header), and only falls back to
   the GET+Basic flow on a narrow status set:
   ```go
   // core/remotes/docker/authorizer.go (v2.1.1, Docker-29 era)
   if (errStatus.StatusCode == 405 && to.Username != "") ||
       errStatus.StatusCode == 404 || errStatus.StatusCode == 401 ||
       errStatus.StatusCode == 400 { /* fall back to FetchToken (GET+Basic) */ }
   ```
   `FetchTokenWithOAuth` POSTs `grant_type=password` and
   `form.Set("username"/"password", …)`; `FetchToken` uses `SetBasicAuth`.
   - https://github.com/containerd/containerd/blob/v2.1.1/core/remotes/docker/authorizer.go
   - https://github.com/containerd/containerd/blob/main/core/remotes/docker/auth/fetch.go
   - Context (403 is *not* a fallback code): https://github.com/containerd/containerd/issues/4982

3. **Podman/skopeo (containers/image) only POST when an identity token exists,
   else GET+Basic:**
   ```go
   if c.auth.IdentityToken != "" { getBearerTokenOAuth2(...) } else { getBearerToken(...) }
   ```
   - https://github.com/containers/image/blob/main/docker/docker_client.go

4. **Bencher's token endpoint is GET-only and reads creds only from the Basic
   header.** Only `GET` + `OPTIONS` are registered at `/v0/auth/oci/token`
   (`lib/api_auth/src/oci/mod.rs:63-90`); `extract_basic_credentials`
   (`:387-422`) parses `Authorization: Basic` and never looks at a form body.
   The project-key branch keys off `password.starts_with("bencher_run_")`
   (`:101`). Dropshot 0.16.5 returns **405** for a method mismatch on a
   registered path (confirmed in `dropshot v0.16.5 router.rs`).

5. **Bencher never issues an identity/refresh token** — `TokenResponse`
   (`:51-60`) returns only `token`/`expires_in`/`issued_at`.

### Why podman works and Docker 29 fails

- **podman**: `IdentityToken == ""` → always GET+Basic → reads the project key
  from the Basic header → **works**.
- **Docker 29 (containerd store)**: POSTs first. Bencher has no POST handler;
  even if it did, the creds are in the form body where Bencher can't read them.

### ⚠️ The ONE empirical question the repro must answer

A bare 405 is *in* containerd's fallback list (when `to.Username != ""`), so in
theory containerd should POST → 405 → fall back to GET → succeed. Yet the user
fails. So the repro must capture **which** of these is true:

- **(a) Edge/proxy rewrites the method-mismatch to a non-fallback code** (e.g.
  a CDN/WAF in front of `registry.bencher.dev` returning **403** on a disallowed
  method). Then containerd never falls back → push dies. *Fix could be as small
  as making the edge return 405/404, OR adding the POST handler.*
- **(b) containerd gets 405 but `to.Username == ""`** at push time (Docker 29
  reworked credential/hostname resolution: release notes mention normalizing
  hostnames in `GetAuthConfig`/`GetCredentialsStore`). Then the 405 branch is
  skipped → push dies. *This is Docker-side; our only in-our-control fix is the
  POST handler (so the first request just succeeds).*
- **(c) Something else** (login itself, TLS, manifest media type).

**Capture the exact token request + status code** (`docker push` against a
`--debug` daemon, or the daemon journal). That decides whether the minimal edge
fix is viable or the POST handler is required. The POST handler is robust to all
of (a)/(b), so it's the recommended fix either way — but record the real failure
mode first so we have a true negative test.

## 3. What was already established in the (network-restricted) sandbox

- Docker **29.3.1** present; started `dockerd` with the **containerd image
  store** (`/etc/docker/daemon.json` → `{"features":{"containerd-snapshotter":true}}`;
  `docker info` shows `driver-type: io.containerd.snapshotter.v1`,
  `containerd-snapshotter=true`). This is the v29 default config that triggers
  the OAuth-POST path. ✅
- **Egress policy blocks all `*.bencher.dev` hosts** (`x-deny-reason:
  host_not_allowed`), so the dev-instance repro could **not** be completed here.
  `ghcr.io` and `registry-1.docker.io` **are** reachable.
- The API server **boots with no config** via `Config::default()`
  (`lib/bencher_config/src/lib.rs:78-103`): SQLite at `data/bencher.db`, binds
  `0.0.0.0:61016`, non-cloud ⇒ registry enabled, `registry_url =
  http://localhost:61016`. `cargo build -p bencher_api --features plus --bin api`
  and `cargo build -p bencher_cli` both succeed.
- No project-key creation helper exists in `bencher_api_tests` (would be useful
  to add for the integration test — see §5).

## 4. Reproduction plan (for the local agent)

Do **Path A** (dev instance, closest to the user's setup). Fall back to
**Path B** (fully local) if dev write access is unavailable.

### Common: enable the containerd image store (the trigger)

```bash
sudo mkdir -p /etc/docker
echo '{ "features": { "containerd-snapshotter": true } }' | sudo tee /etc/docker/daemon.json
sudo systemctl restart docker   # or: sudo pkill dockerd && sudo dockerd &
docker info | grep -iE 'server version|snapshotter|driver-type'   # confirm v29 + io.containerd.snapshotter.v1
```

### Path A — against `dev.registry.bencher.dev`

Repo-committed dev credentials (NOT secrets — already in
`tasks/test_api/src/task/test/smoke_test.rs`):
- Dev API URL: `https://dev.api.bencher.dev`
- Dev registry: `https://dev.registry.bencher.dev`
- Project slug: `the-computer`
- `DEV_ADMIN_BENCHER_API_TOKEN` (the long JWT in `smoke_test.rs:26`).

```bash
# 1) Create a project key the way CI does (see run_project_key_runner_test)
KEY=$(cargo run -p bencher_cli -- project key create \
  --host https://dev.api.bencher.dev --token "$DEV_ADMIN_TOKEN" \
  --name repro-docker29 the-computer | jq -r .key)   # → bencher_run_…

# 2) Log in with the project SLUG as username and the key as password
echo "$KEY" | docker login dev.registry.bencher.dev -u the-computer --password-stdin

# 3) Get any image locally (ghcr is reachable), tag for the project, push
docker pull --platform linux/amd64 ghcr.io/bencherdev/bencher:latest
docker tag ghcr.io/bencherdev/bencher:latest dev.registry.bencher.dev/the-computer:repro-docker29
docker push dev.registry.bencher.dev/the-computer:repro-docker29   # EXPECT FAILURE on Docker 29
```

**Capture the failure mode** (this is the negative test data):
```bash
# Run dockerd with --debug (or read the journal) and re-run the push:
sudo dockerd --debug >/tmp/dockerd-debug.log 2>&1 &
docker push dev.registry.bencher.dev/the-computer:repro-docker29 2>&1 | tee /tmp/push.log
grep -iE 'token|oauth|POST|GET|401|403|405|www-authenticate|realm' /tmp/dockerd-debug.log
```
Record: the exact request method to `…/v0/auth/oci/token`, the status code
returned, and whether containerd fell back to GET. **This answers §2's (a)/(b)/(c).**

**A/B isolation (the clean "compare against"):** with the *same* Docker 29
binary, disable the containerd store (remove the feature from
`/etc/docker/daemon.json`, restart) and repeat steps 2-3. Expectation:
**legacy store push succeeds, containerd-store push fails** → proves the image
store / OAuth-POST path is the variable. (Also: `podman login` + `podman push`
with the same creds should succeed.)

### Path B — fully local (no external bencher network needed)

```bash
# Terminal 1: run the API+registry on :61016 with defaults
cd <repo>; mkdir -p data
cargo run -p bencher_api --features plus --bin api    # serves http://localhost:61016 (HTTP ⇒ docker treats localhost as insecure)

# Terminal 2: seed user/org/project, then create a key (CLI builds as `bencher`)
cargo test-api seed --host http://localhost:61016     # creates the-computer project + muriel.bagge user
# (or drive `bencher auth signup` / `org create` / `project create` manually)
KEY=$(cargo run -p bencher_cli -- project key create \
  --host http://localhost:61016 --token "$USER_TOKEN" --name repro the-computer | jq -r .key)

echo "$KEY" | docker login localhost:61016 -u the-computer --password-stdin
docker tag ghcr.io/bencherdev/bencher:latest localhost:61016/the-computer:repro
docker push localhost:61016/the-computer:repro        # observe behavior; check /tmp/bencher_api.log for the token-endpoint hit
```
The server log will show whether `/v0/auth/oci/token` was hit with GET vs POST
and what it returned — directly confirming the server-side half.

### Cheap server-side confirmation (no docker needed)

Simulate exactly what each client sends, against a running server (local or dev):
```bash
REALM=http://localhost:61016/v0/auth/oci/token   # or https://dev.registry.bencher.dev/v0/auth/oci/token
SCOPE='repository:the-computer:push'

# podman/GET style (expect 200 + {"token":...}):
curl -sS -i -u "the-computer:$KEY" "$REALM?scope=$SCOPE&service=localhost"

# containerd/POST style (expect 405 today; the bug):
curl -sS -i -X POST "$REALM" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode grant_type=password \
  --data-urlencode service=localhost \
  --data-urlencode client_id=containerd-client \
  --data-urlencode "scope=$SCOPE" \
  --data-urlencode username=the-computer \
  --data-urlencode "password=$KEY"
```
This pair *is* the negative test in miniature: GET authenticates, POST does not.

## 5. The fix — add OAuth2 POST to the OCI token endpoint

Make the POST-first clients authenticate identically to the GET clients. Keep a
**single auth code path**: one shared dispatch helper, two thin method wrappers.

### 5.1 `lib/api_auth/src/oci/mod.rs`

- **Extract shared dispatch.** Pull the credential-branching currently inside
  `auth_oci_token_get` (`:101-144` — the `bencher_run_` prefix → `project_key_oci_token`
  vs email/JWT → `auth_oci_token` vs anonymous) into a private
  `async fn issue_oci_token(rqctx, context, query/scope, repository, actions,
  creds: Option<(String,String)>) -> Result<Jwt, HttpError>`. Have the existing
  GET handler call it. **Reuse** `project_key_oci_token` (`:312-380`),
  `auth_oci_token` (`:163-257`), `parse_scope` (`:428-463`),
  `unauthorized_with_www_authenticate` (`:264-301`).
- **Add `POST /v0/auth/oci/token`.** Accept an `application/x-www-form-urlencoded`
  body. Dropshot: take the raw body (`UntypedBody`) and parse with
  `serde_urlencoded` into a struct:
  ```rust
  struct OAuthTokenForm {
      grant_type: String,          // "password" (supported) | "refresh_token" (reject)
      service: Option<String>,
      scope: Option<String>,       // reuse parse_scope
      client_id: Option<String>,
      username: Option<String>,
      password: Option<String>,
      refresh_token: Option<String>,
      access_type: Option<String>,
  }
  ```
  - `grant_type=password`: build `creds = Some((username, password))` and call the
    shared `issue_oci_token`. The `bencher_run_` prefix branch then routes to
    `project_key_oci_token` exactly like the GET path.
  - `grant_type=refresh_token` (or missing creds): Bencher issues no refresh
    tokens → return OAuth2 `400 invalid_request` / `unsupported_grant_type`.
- **Response shape differs.** The OAuth2 POST response field is **`access_token`**,
  not `token` (https://distribution.github.io/distribution/spec/auth/oauth/). Add:
  ```rust
  struct OAuthTokenResponse { access_token: String, expires_in: u32,
                              scope: Option<String>, issued_at: String }
  ```
  Keep the GET `TokenResponse { token, … }` unchanged (spec-correct for GET).
- **OPTIONS / CORS**: include `Post` alongside `Get` in `auth_oci_token_options`
  (`:68-72`) if browsers ever hit it; otherwise leave as-is.
- Add `serde_urlencoded` to the workspace `Cargo.toml` if not already present
  (`cargo tree -p api_auth | grep serde_urlencoded`), via `dep.workspace = true`.

### 5.2 Regenerate the API surface

```bash
cargo gen-types      # updates services/api/openapi.json + console TS types
```
Commit the regenerated `services/api/openapi.json`.

### 5.3 Follow the repo conventions

- Strong types over `String`/`Value` where practical; `thiserror`, no `anyhow`
  in `lib/`; `#[expect(...)]` not `#[allow(...)]`; `cargo fmt` + the full clippy
  line from `CLAUDE.md`; `cargo check --no-default-features`.
- Track auth failures via `bencher_otel::ApiMeter::increment` if the GET path
  already does, to stay consistent.

## 6. Test plan

### 6.1 Unit (`lib/api_auth/src/oci/mod.rs` `mod tests`)
- `serde_urlencoded` parsing of the form (happy path, missing `grant_type`,
  `refresh_token` grant → error).
- `grant_type=password` with a `bencher_run_…` password dispatches to the
  project-key branch; with email + JWT dispatches to the user branch.
- Response serializes `access_token` (not `token`).

### 6.2 Integration (`plus/api_oci/tests/token.rs`)
Mirror every existing GET case with a POST-form counterpart:
- anonymous-ish / missing creds → 400 (POST has no anonymous mode).
- `grant_type=password` project-key (need a created project key — see 6.4) →
  200 + `access_token`, and that token authorizes `GET /v2/`.
- `grant_type=password` email+api-key → 200 + `access_token`.
- wrong password → 401 with `WWW-Authenticate`.
- `grant_type=refresh_token` → 400.

### 6.3 Real-docker e2e negative→positive (the "compare against" test)
Model on `run_project_key_runner_test`
(`tasks/test_api/src/task/plus/runner.rs:574-674`). Add a variant that runs with
the **containerd image store enabled** and asserts:
- **Before the fix**: project-key `docker push` fails (capture status). Mark
  `#[ignore]` / gate behind an env flag so CI without v29+containerd skips it.
- **After the fix**: the same push succeeds.
Run the existing test on both store backends to prove parity.

### 6.4 Add a project-key test helper
In `bencher_api_tests` add `TestServer::create_project_key(&user, &project,
name) -> ProjectKey` (hit `POST /v0/projects/{slug}/keys`, return
`JsonProjectKeyCreated.key`) so 6.2/6.3 don't hand-roll it.

### 6.5 Commands
```bash
cargo nextest run -p api_auth --features server
cargo test --doc -p api_auth
cargo nextest run -p api_oci --features plus
cargo gen-types && git diff --exit-code services/api/openapi.json   # ensure committed
cargo clippy --no-deps --all-targets --all-features -- -Dwarnings
cargo fmt
```

## 7. Future steps / docs (the user explicitly asked for a docs mention)

- **Docs note** in all 9 i18n copies of
  `services/console/src/chunks/docs-tutorial/bare-metal/<lang>/push-image.mdx`
  (`en,de,es,fr,ja,ko,pt,ru,zh`): if `docker push` 401/403s with a project key on
  some Docker builds, upgrade the Bencher server (OAuth2 token support) or use
  `podman` as a fallback. Bump `modified:` on the parent content page
  `services/console/src/content/docs-tutorial/<lang>/bare-metal.mdx`
  (per `services/console/CLAUDE.md` i18n + modified-date rules).
- **Changelog**: add a `## Pending` entry in
  `services/console/src/chunks/docs-reference/changelog/en/changelog.mdx`
  ("OCI token endpoint now supports the OAuth2 `POST` flow for Docker 29+ /
  containerd image store").
- **Edge check**: confirm what the edge in front of `registry.bencher.dev`
  returns for `POST /v0/auth/oci/token` (the repro's status capture). If it's
  403/blocked, ensure the new POST route is allowed through to the API.
- **Branch / PR**: develop on `claude/project-key-auth-docker-0hr0j`; PRs target
  `devel`.

## 8. Critical files

- `lib/api_auth/src/oci/mod.rs` — POST handler, `OAuthTokenResponse`, shared
  `issue_oci_token`; reuse `project_key_oci_token`, `auth_oci_token`, `parse_scope`.
- `plus/api_oci/tests/token.rs` — POST integration coverage.
- `lib/bencher_api_tests/src/oci.rs` — add `create_project_key` helper.
- `tasks/test_api/src/task/plus/runner.rs` — containerd-store e2e variant.
- `services/api/openapi.json` (+ generated TS) — regenerated.
- `services/console/src/chunks/docs-tutorial/bare-metal/*/push-image.mdx`,
  `services/console/src/content/docs-tutorial/*/bare-metal.mdx`,
  `services/console/src/chunks/docs-reference/changelog/en/changelog.mdx` — docs.

## 9. Sources

- Docker v29 containerd default: https://www.docker.com/blog/docker-engine-version-29/ , https://docs.docker.com/engine/storage/containerd/ , https://github.com/moby/moby/issues/51532
- containerd OAuth-POST-first + fallback codes: https://github.com/containerd/containerd/blob/v2.1.1/core/remotes/docker/authorizer.go , https://github.com/containerd/containerd/blob/main/core/remotes/docker/auth/fetch.go , https://github.com/containerd/containerd/issues/4982
- podman/containers-image GET-unless-IdentityToken: https://github.com/containers/image/blob/main/docker/docker_client.go
- OAuth2 token spec (`access_token`): https://distribution.github.io/distribution/spec/auth/oauth/
- token spec (GET, `token`): https://distribution.github.io/distribution/spec/auth/token/
- Dropshot 405 router: https://github.com/oxidecomputer/dropshot/blob/v0.16.5/dropshot/src/router.rs
