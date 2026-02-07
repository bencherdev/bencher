# Code Review: OCI Registry Implementation (since `main`)

## Overview

This is a major feature addition (~11K lines, 111 files, 7 commits) that adds an **OCI Distribution Spec compliant container registry** to the Bencher API server. The changes include:

- **`plus/api_oci`** - Dropshot endpoints for blobs, manifests, tags, referrers, and uploads
- **`plus/bencher_oci_storage`** - Dual storage backend (local filesystem + S3 via Access Points)
- **`lib/api_auth/src/oci`** - OCI token authentication endpoint
- **`lib/bencher_token`** - New `Oci` audience type and OCI-scoped JWT claims
- **`lib/bencher_config`** - Registry configuration with S3 data store support
- CI/CD, Dockerfile, and documentation updates

The code is generally well-structured with clear separation of concerns, good documentation, and consistent patterns across endpoint files.

---

## High Severity

### 1. Missing cross-repository mount authorization (`plus/api_oci/src/blobs.rs:293-324`)
When `oci_upload_start` processes a cross-repository mount, it validates push access to the *target* repository but does **not** validate pull access to the *source* repository (`query.from`). An attacker with push access to their own repository could mount blobs from any other repository without pull authorization. The `storage.mount_blob` call passes `&from_repo` without any auth check on that repository.

### 2. Silent auth failure downgrades to public access (`plus/api_oci/src/auth.rs:447-469`)
In `build_public_user`, if OCI token validation fails or the user is not found, the function silently falls through to treating the request as a public (unauthenticated) user. A request with an *invalid* or *expired* token is treated identically to one with *no* token. For unclaimed projects, this still grants push access.

---

## Medium Severity

### 3. No upload size limits on monolithic uploads/chunks (`plus/api_oci/src/blobs.rs:373-438`, `uploads.rs:172`)
`UntypedBody` buffers the entire request body in memory with no explicit size check. An attacker could send a very large body to exhaust server memory. Additionally, `Bytes::copy_from_slice(data)` doubles the memory pressure by copying data again.

### 4. `Digest::sha256()` bypasses `FromStr` validation (`plus/bencher_oci_storage/src/types.rs:22-25`)
The public `Digest::sha256()` constructor creates a Digest without validation, contradicting the safety comment on `algorithm()` that says "Digest is only constructed via FromStr which validates format." While callers always pass hex-encoded SHA-256 output, the API is unsound.

### 5. `OciError::Storage(_)` always maps to `BLOB_UNKNOWN` (`plus/bencher_oci_storage/src/error.rs:63`)
All `OciStorageError` variants are mapped to the `BLOB_UNKNOWN` OCI error code when wrapped in `OciError::Storage`. A `ManifestNotFound` should map to `MANIFEST_UNKNOWN`, and a `DigestMismatch` should map to `DIGEST_INVALID`.

### 6. Silent referrer data corruption (`plus/bencher_oci_storage/src/storage.rs:1419`, `local.rs:630`)
`serde_json::to_vec(&descriptor).unwrap_or_default()` silently stores empty data if serialization fails, creating corrupt referrer links that will be silently skipped during listing.

### 7. Session-based upload auth has no revocation/user-binding/expiry (`plus/api_oci/src/uploads.rs:25-29`)
Upload session endpoints rely solely on UUID unguessability. There is no way to revoke a session, no binding between session and the creating user, and session expiry behavior is unclear.

### 8. Header injection via `scope` in `WWW-Authenticate` (`lib/api_auth/src/oci/mod.rs:242`)
The scope string from user input is interpolated into the `WWW-Authenticate` header without sanitizing `"` characters, allowing potential header directive injection.

---

## Low Severity

### 9. Missing `self.google.sanitize()` in `JsonPlus::sanitize()` (`lib/bencher_json/src/system/config/plus/mod.rs:44-48`)
`JsonGoogle` has a `client_secret: Secret` and implements `Sanitize`, but the `JsonPlus::sanitize()` method does not call `self.google.sanitize()`. This could leak the Google OAuth client secret in sanitized config output. **Pre-existing bug**, not introduced by this change set, but discovered during review.

### 10. No maximum page size cap for tags (`plus/api_oci/src/tags.rs:107`)
`query.n` (a `u32`) is used directly with no upper bound, allowing a client to request ~4 billion tags.

### 11. S3 stale upload cleanup doesn't paginate (`plus/bencher_oci_storage/src/storage.rs:1854-1863`)
Only the first 1000 upload prefixes (S3 default page size) are cleaned. Older stale uploads beyond that are never cleaned until earlier ones are removed.

### 12. Case-sensitive Basic auth scheme comparison (`lib/api_auth/src/oci/mod.rs:281`)
`scheme != "Basic"` should be case-insensitive per RFC 7235.

### 13. No repository binding validation for upload sessions (`plus/api_oci/src/uploads.rs`)
Session operations don't verify the session belongs to the repository in the URL path.

### 14. `OciScopeClaims.actions` is `Vec<String>` not an enum (`lib/bencher_token/src/claims.rs:29-30`)
No compile-time enforcement that actions are only `"pull"` or `"push"`.

### 15. Manifest content not validated against OCI schema (`plus/api_oci/src/manifests.rs:220-234`)
Arbitrary JSON can be stored as a manifest. The registry could be abused as a generic key-value store.

### 16. `tag_path` in local storage accepts raw `&str` (`plus/bencher_oci_storage/src/local.rs:181-183`)
Defense-in-depth concern: `PathBuf::join(tag)` with a raw `&str` could enable path traversal if the upstream `Tag` validation is ever bypassed. Mitigated by current caller validation.

---

## Informational

- **No unit tests for OCI token roundtrip** in `bencher_token/src/key.rs` (only integration-level tests exist)
- **Duplicated content-type extraction** in `manifests.rs` (lines 85-92 and 157-164) -- extract to helper
- **Duplicated S3 404 error mapping** pattern repeated ~7 times in `storage.rs` -- extract to helper
- **Empty `OCI-Filters-Applied` header** when no filter is applied (`referrers.rs:105`) -- should be omitted per spec
- **`append_upload` re-lists all chunks** after every append, making it O(n) per append
- WAL mode is now applied unconditionally (beneficial change but behavioral difference from pre-existing code)

---

## Positive Observations

- Well-documented module-level comments and auth flow explanations
- Clean helper function decomposition for auth (claimed/unclaimed/nonexistent project handling)
- Proper `#[expect(...)]` usage with reason strings
- Defense-in-depth JWT expiration double-check in `TokenKey::validate`
- Correct audience separation between token types
- Good fallback to local filesystem storage for self-hosted deployments
- Proper `Sanitize` implementation for S3 credentials
- Comprehensive integration test suite covering base, blobs, manifests, referrers, tags, and uploads
