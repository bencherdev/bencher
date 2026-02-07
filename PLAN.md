# Plan: OCI Registry Fixes

## Answers to Your Questions

**Issue 3 (Upload session expiry):** The OCI spec says servers "SHOULD eventually timeout unfinished uploads" (SHOULD, not MUST). Since the storage layer already implements stale upload cleanup via `spawn_stale_upload_cleanup()` on every `start_upload`, this is already handled. **No change needed.**

**Issue 7 (ManifestInvalid for bad reference):** The spec doesn't define a specific error code for invalid reference strings. The most defensible choice from the OCI error code table is `MANIFEST_INVALID` with HTTP 400, since it's a manifest operation with invalid input. However, the current code uses `ManifestInvalid` for the PUT (bad reference) but `ManifestUnknown` for GET/HEAD/DELETE. For consistency and correctness: PUT should also use `ManifestUnknown` (404) for a bad reference — a bad reference means the manifest referenced doesn't exist/can't be found, same as GET/HEAD/DELETE. This matches what registries like GHCR do in practice.

**Issue 10 (Manifest schema):** Yes, the OCI Image Spec defines well-typed schemas for all four manifest types. We should create strongly-typed Rust structs for OCI Descriptor, OCI Image Manifest, OCI Image Index, Docker Manifest V2, and Docker Manifest List. These will replace the `serde_json::Value` usage for validation and content-type extraction.

---

## Changes

### 1. OCI error responses — `plus/api_oci/src/error.rs`

The OCI spec says: if the response body is JSON, it MUST follow `{"errors": [{"code": "...", "message": "...", "detail": ...}]}`.

**Change `into_http_error`** to produce OCI-compliant JSON error bodies:
- Use `OciError::code()` to get the error code string
- Use `OciError::to_string()` for the message
- Format as `{"errors": [{"code": "<CODE>", "message": "<msg>"}]}`
- Set the `external_message` to this JSON string
- Also update `payload_too_large` to use the same OCI format (with code `SIZE_INVALID`)

### 2. Digest hash length validation — `plus/bencher_oci_storage/src/types.rs`

**In `Digest::sha256()`** (line 23-28): Add length check `hex_hash.len() == 64`.

**In `Digest::from_str()`** (line 57-76): Add length validation after algorithm check:
- `sha256` → encoded must be 64 chars
- `sha512` → encoded must be 128 chars

**Update tests** to cover:
- Valid 64-char sha256 passes
- Short sha256 (`sha256:abc`) is rejected
- Long sha256 is rejected
- Valid 128-char sha512 passes
- Short/long sha512 is rejected

### 3. Upload session expiry — NO CHANGE

Already handled by `spawn_stale_upload_cleanup()`. Spec only says SHOULD, not MUST.

### 4. RBAC error confirms resource existence — `plus/api_oci/src/auth.rs:300-308`

Change the FORBIDDEN error message from:
```
"Access denied to repository: {slug}. You need Create permission."
```
to a generic:
```
"Insufficient permissions"
```

This prevents enumeration — attackers can no longer distinguish "project exists but no access" from "project doesn't exist".

### 5. ManifestInvalid → ManifestUnknown for bad reference in PUT — `plus/api_oci/src/manifests.rs:212-214`

Change the error for a bad reference parse in `oci_manifest_put` from `OciError::ManifestInvalid` to `OciError::ManifestUnknown`, matching the GET/HEAD/DELETE pattern:

```rust
let reference: Reference = path.reference.parse().map_err(|_err| {
    crate::error::into_http_error(OciError::ManifestUnknown {
        reference: path.reference.clone(),
    })
})?;
```

### 6. S3 cleanup order — `plus/bencher_oci_storage/src/storage.rs:858-893`

Reorder `cleanup_upload` to delete chunks BEFORE the state file:
1. Delete buffer chunks (list then delete each)
2. Delete multipart data object
3. Delete state file (last, so discovery still works if crash occurs mid-cleanup)

### 7. Content-Type preservation and manifest types — `plus/api_oci/src/manifests.rs` + new types

The OCI spec says the registry SHOULD return the correct `Content-Type` on GET. Since the body is stored verbatim and `mediaType` is preserved in the JSON body, inferring from the body is spec-compliant. However, we should make this more robust by using strongly-typed manifest schemas.

**Create `lib/bencher_json/src/oci/manifest.rs`** (gated behind `#[cfg(feature = "plus")]`) with typed manifest structs:

Since these types represent the shape of data coming in through the API, they belong in `bencher_json`.
They don't need `#[typeshare]` (not shared with frontend) and are gated behind the `plus` feature.

New module: `lib/bencher_json/src/oci/mod.rs` — re-exports manifest types.

```rust
/// OCI Content Descriptor (shared across all manifest types)
struct OciDescriptor {
    media_type: String,      // REQUIRED
    digest: String,          // REQUIRED
    size: i64,               // REQUIRED
    urls: Option<Vec<String>>,
    annotations: Option<HashMap<String, String>>,
    data: Option<String>,
    artifact_type: Option<String>,
}

/// OCI Image Manifest
struct OciImageManifest {
    schema_version: u32,     // REQUIRED, must be 2
    media_type: Option<String>,
    config: OciDescriptor,   // REQUIRED
    layers: Vec<OciDescriptor>, // REQUIRED
    subject: Option<OciDescriptor>,
    annotations: Option<HashMap<String, String>>,
    artifact_type: Option<String>,
}

/// OCI Image Index
struct OciImageIndex {
    schema_version: u32,
    media_type: Option<String>,
    manifests: Vec<OciManifestDescriptor>,
    subject: Option<OciDescriptor>,
    annotations: Option<HashMap<String, String>>,
    artifact_type: Option<String>,
}

/// Platform info for index/list entries
struct Platform {
    architecture: String,
    os: String,
    os_version: Option<String>,
    os_features: Option<Vec<String>>,
    variant: Option<String>,
}

/// Manifest descriptor with optional platform (for index/list)
struct OciManifestDescriptor {
    #[serde(flatten)]
    descriptor: OciDescriptor,
    platform: Option<Platform>,
}

/// Docker Manifest V2 Schema 2
struct DockerManifestV2 {
    schema_version: u32,
    media_type: String,
    config: OciDescriptor,
    layers: Vec<OciDescriptor>,
}

/// Docker Manifest List
struct DockerManifestList {
    schema_version: u32,
    media_type: String,
    manifests: Vec<OciManifestDescriptor>,
}

/// Enum wrapping all supported manifest types
enum Manifest {
    OciImageManifest(OciImageManifest),
    OciImageIndex(OciImageIndex),
    DockerManifestV2(DockerManifestV2),
    DockerManifestList(DockerManifestList),
}
```

**Wire up in `bencher_json`:**
- Add `#[cfg(feature = "plus")] pub mod oci;` to `lib/bencher_json/src/lib.rs`
- Re-export types from `lib/bencher_json/src/oci/mod.rs`

**Update `manifests.rs`:**
- Replace `serde_json::from_slice::<serde_json::Value>` with typed deserialization using `bencher_json::oci::Manifest`
- Try to deserialize as `Manifest` enum based on `mediaType` field
- `extract_content_type` uses the typed manifest's `media_type` field
- Validation is now structural — missing required fields like `config` or `layers` are caught
- Subject extraction uses the typed `subject` field
- If deserialization fails entirely, return `ManifestInvalid`

**Update `plus/bencher_oci_storage/src/types.rs`:**
- Move `extract_subject_digest` and `build_referrer_descriptor` to use the new `bencher_json::oci` typed manifests
- The `build_referrer_descriptor` function can now access fields directly instead of through `serde_json::Value` navigation

### 8. CORS helper — `plus/api_oci/src`

Create a helper function in a shared location within `plus/api_oci/src` (add to existing `error.rs` or create a small `response.rs` module) to add OCI CORS headers to a `Response::builder()`:

```rust
/// Adds standard OCI CORS headers to a response builder
pub fn oci_cors_headers(
    builder: http::response::Builder,
    methods: &str,
) -> http::response::Builder {
    builder
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, methods)
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
}
```

Replace all 17 manual CORS header insertions across `base.rs`, `blobs.rs`, `manifests.rs`, `uploads.rs`, `referrers.rs` with calls to this helper. This ensures consistency (some endpoints currently omit `ACCESS_CONTROL_ALLOW_METHODS` or `ACCESS_CONTROL_ALLOW_HEADERS`).

### 9. Duplicated reference resolution — `plus/api_oci/src/manifests.rs`

Extract the reference resolution pattern (lines 89-96, 151-157) into a helper:

```rust
async fn resolve_reference(
    storage: &OciStorage,
    name: &ProjectResourceId,
    reference: &Reference,
) -> Result<Digest, HttpError> {
    match reference {
        Reference::Digest(d) => Ok(d.clone()),
        Reference::Tag(t) => storage
            .resolve_tag(name, t)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e))),
    }
}
```

Use in both `oci_manifest_exists` and `oci_manifest_get`.

### 10. Redundant `.clone()` — `plus/api_oci/src/auth.rs:364`

Change `match repository.clone()` to match on a reference:

```rust
match repository {
    ProjectResourceId::Uuid(uuid) => {
        slog::debug!(log, "OCI push to non-existent project by UUID"; "uuid" => %uuid);
        Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::NOT_FOUND,
            format!("Project not found: {uuid}"),
        ))
    },
    ProjectResourceId::Slug(slug) => {
        // slug is borrowed, use as_ref() for the string conversion
        let slug_str: &str = slug.as_ref();
        // ...rest unchanged...
    },
}
```

---

## File Change Summary

| File | Change |
|------|--------|
| `plus/api_oci/src/error.rs` | OCI-compliant JSON error bodies |
| `plus/bencher_oci_storage/src/types.rs` | Digest length validation |
| `plus/api_oci/src/auth.rs` | Generic RBAC error message (line 300-308), remove redundant clone (line 364) |
| `plus/api_oci/src/manifests.rs` | Fix reference error type, use typed manifests, extract resolve helper, use CORS helper |
| `plus/bencher_oci_storage/src/storage.rs` | Reorder cleanup: chunks before state |
| `lib/bencher_json/src/oci/mod.rs` | NEW: OCI module re-exports |
| `lib/bencher_json/src/oci/manifest.rs` | NEW: Typed OCI/Docker manifest structs |
| `lib/bencher_json/src/lib.rs` | Add `#[cfg(feature = "plus")] pub mod oci;` |
| `plus/api_oci/src/response.rs` | NEW: CORS helper function |
| `plus/api_oci/src/base.rs` | Use CORS helper |
| `plus/api_oci/src/blobs.rs` | Use CORS helper |
| `plus/api_oci/src/uploads.rs` | Use CORS helper |
| `plus/api_oci/src/referrers.rs` | Use CORS helper |
| `plus/api_oci/src/tags.rs` | Use CORS helper |
| `plus/api_oci/src/lib.rs` | Add `mod response;` (or `mod manifest;` if in storage) |

## Execution Order

1. **Manifest types** (issue 7/9/10) — foundation for validation changes
2. **Error format** (issue 1) — foundational, affects all endpoints
3. **Digest validation** (issue 2) — isolated change
4. **CORS helper** (issue 16) — extract pattern, then use in subsequent changes
5. **Manifest fixes** (issues 5, 7, 9, 17) — use new types + CORS helper + resolve helper
6. **Auth fixes** (issues 4, 18) — simple isolated changes
7. **S3 cleanup order** (issue 8) — isolated storage change
8. Run `cargo fmt`, `cargo clippy`, `cargo check --no-default-features`, `cargo test`
9. Run `cargo gen-types` if the OpenAPI spec changed (it shouldn't since these are `unpublished` OCI endpoints, but verify)
