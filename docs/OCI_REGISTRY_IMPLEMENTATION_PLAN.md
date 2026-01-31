# OCI Container Registry Implementation Plan

This document provides a step-by-step guide for implementing an OCI-compliant container registry push workflow in the Bencher API server.

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Phase 1: Project Setup](#phase-1-project-setup)
4. [Phase 2: Core Types and Storage](#phase-2-core-types-and-storage)
5. [Phase 3: Base Endpoint](#phase-3-base-endpoint)
6. [Phase 4: Blob Push Endpoints](#phase-4-blob-push-endpoints)
7. [Phase 5: Manifest Push Endpoint](#phase-5-manifest-push-endpoint)
8. [Phase 6: Pull Endpoints](#phase-6-pull-endpoints)
9. [Phase 7: Content Discovery](#phase-7-content-discovery)
10. [Phase 8: Content Management](#phase-8-content-management)
11. [Phase 9: OTEL Metrics](#phase-9-otel-metrics)
12. [Phase 10: Conformance Testing](#phase-10-conformance-testing)
13. [Phase 11: CI Integration](#phase-11-ci-integration)
14. [Reference](#reference)

---

## Overview

### What We're Building

An OCI Distribution Spec compliant container registry that:
- Implements the push workflow for uploading container images
- Passes the official OCI conformance tests
- Integrates with the existing Bencher API server (Dropshot framework)
- Emits metrics via OpenTelemetry (OTEL)
- **Is a Bencher Plus feature** (gated behind the `plus` feature flag)

### OCI Distribution Spec Workflows

The spec defines four workflow categories (in priority order):

| Workflow           | Priority | Description                               |
| ------------------ | -------- | ----------------------------------------- |
| Pull               | Highest  | Retrieve content from registry (REQUIRED) |
| Push               | High     | Upload content to registry                |
| Content Discovery  | Medium   | List tags and referrers                   |
| Content Management | Lower    | Delete content                            |

**Important**: All registries MUST support Pull. To pass conformance tests, we need to implement all four workflows.

### Key Endpoints Summary

| Method   | Endpoint                           | Purpose              |
| -------- | ---------------------------------- | -------------------- |
| GET      | `/v2/`                             | API version check    |
| HEAD/GET | `/v2/<name>/blobs/<digest>`        | Check/fetch blob     |
| POST     | `/v2/<name>/blobs/uploads/`        | Initiate blob upload |
| PATCH    | `<location>`                       | Upload blob chunk    |
| PUT      | `<location>?digest=<digest>`       | Complete blob upload |
| DELETE   | `<location>`                       | Cancel upload        |
| HEAD/GET | `/v2/<name>/manifests/<reference>` | Check/fetch manifest |
| PUT      | `/v2/<name>/manifests/<reference>` | Upload manifest      |
| GET      | `/v2/<name>/tags/list`             | List tags            |
| GET      | `/v2/<name>/referrers/<digest>`    | List referrers       |
| DELETE   | `/v2/<name>/blobs/<digest>`        | Delete blob          |
| DELETE   | `/v2/<name>/manifests/<digest>`    | Delete manifest      |

### Storage Architecture

The registry uses **AWS S3** for blob and manifest storage, leveraging the same `aws-sdk-s3` library already used for database backups. This enables:
- Horizontal scaling across multiple API server instances
- Integration with Bencher Cloud infrastructure
- Consistent configuration patterns with existing backup functionality

**S3 Object Layout:**
```
<access-point>/
├── oci/
│   ├── blobs/
│   │   └── sha256/
│   │       └── ab/                  # First 2 chars of digest hex
│   │           └── abcd1234...      # Full digest hex -> blob content
│   ├── manifests/
│   │   └── <repo>/
│   │       ├── <digest>.json        # Manifest content
│   │       ├── <digest>.meta        # Manifest metadata
│   │       └── _tags/
│   │           └── <tag>            # Tag -> digest mapping
│   └── uploads/
│       └── <session-id>/
│           └── data                 # In-progress upload data
```

**Configuration:** Configured via `plus.oci` section in `bencher.json`, using the same ARN-based S3 access point pattern as database backups.

---

## Prerequisites

Before starting, ensure you understand:

1. **Rust fundamentals** - Ownership, lifetimes, async/await
2. **Dropshot framework** - Review existing endpoints in `lib/api_*/`
3. **OCI concepts** - Blobs, manifests, digests, tags
4. **The Bencher codebase** - Read `CLAUDE.md` for project structure

### Required Reading

- [OCI Distribution Spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md)
- [Dropshot Documentation](https://docs.rs/dropshot)
- Existing Bencher API patterns in `lib/api_server/src/`
- Existing Plus features in `plus/` directory (e.g., `bencher_billing`, `bencher_license`)

### Building with Plus Features

The OCI registry requires the `plus` feature flag. When building and testing:

```bash
# Build with Plus features (default)
cargo build

# Build without Plus features (OCI registry will NOT be included)
cargo build --no-default-features

# Run tests with Plus features
cargo test

# Run the API server with Plus features
cd services/api && cargo run
```

---

## Phase 1: Project Setup

> **Note**: The OCI registry is a **Bencher Plus** feature. All code should be placed in the `plus/` directory and gated behind the `plus` feature flag.

### Step 1.1: Create the New Library Crate

Create a new library for OCI registry endpoints in the `plus/` directory (following the pattern of other Plus features like `bencher_billing`, `bencher_license`, etc.).

```bash
# From repository root
mkdir -p plus/bencher_oci/src
```

Create `plus/bencher_oci/Cargo.toml`:

```toml
[package]
name = "bencher_oci"
version.workspace = true
authors.workspace = true
edition.workspace = true

[dependencies]
# Bencher internal crates
bencher_endpoint.workspace = true
bencher_json.workspace = true
bencher_schema = { workspace = true, features = ["plus"] }
bencher_valid.workspace = true

# AWS S3 (same as database backup)
aws-credential-types.workspace = true
aws-sdk-s3.workspace = true

# External dependencies
bytes.workspace = true
camino.workspace = true
dropshot.workspace = true
hex.workspace = true
http.workspace = true
regex.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["fs", "io-util", "sync"] }
tracing.workspace = true
uuid.workspace = true

# Plus-only dependencies
bencher_otel = { path = "../bencher_otel", optional = true }

[features]
default = ["sentry", "otel"]
sentry = ["bencher_schema/sentry"]
otel = ["dep:bencher_otel", "bencher_schema/otel"]
```

> **Why `plus/` directory?** Bencher Plus features are organized in the `plus/` directory to clearly separate commercial features from the open-source core. This follows the existing pattern used by `bencher_billing`, `bencher_license`, `bencher_otel`, etc.

> **Why `aws-sdk-s3`?** This is the same library already used for database backups, ensuring consistent S3 access patterns and credential handling across the codebase.

### Step 1.2: Create the Library Entry Point

Create `plus/bencher_oci/src/lib.rs`:

```rust
//! Bencher OCI Registry - A Bencher Plus Feature
//!
//! This module implements an OCI Distribution Spec compliant container registry
//! that integrates with the Bencher API server.

mod endpoints;
mod error;
mod storage;
mod types;

pub use endpoints::register;
pub use storage::{BlobStore, ManifestStore, OciStorage, StorageConfig, UploadStore};

/// Register all OCI registry endpoints with the API server.
///
/// This function is called from the main API registration when the `plus` feature is enabled.
pub fn register_endpoints(
    api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
    http_options: bool,
) -> Result<(), dropshot::ApiDescriptionRegisterError> {
    endpoints::register(api_description, http_options)
}
```

### Step 1.3: Add to Workspace

Update the root `Cargo.toml` workspace members:

```toml
[workspace]
members = [
    # ... existing members
    "plus/bencher_oci",
]
```

### Step 1.4: Register with API Server (Plus Feature Gated)

Update `services/api/Cargo.toml` to include the new crate **conditionally**:

```toml
[dependencies]
# ... existing dependencies

[dependencies.bencher_oci]
path = "../../plus/bencher_oci"
optional = true

[features]
default = ["plus", "sentry", "otel"]
plus = [
    # ... existing plus dependencies
    "dep:bencher_oci",
]
```

Update `services/api/src/api.rs` to register the OCI endpoints **only when `plus` is enabled**:

```rust
impl Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // ... existing registrations

        // OCI Registry - Plus feature only
        #[cfg(feature = "plus")]
        bencher_oci::register_endpoints(api_description, http_options)?;

        Ok(())
    }
}
```

> **Important**: The `#[cfg(feature = "plus")]` attribute ensures the OCI registry code is only compiled and registered when building with Plus features enabled. Without `--features plus` (or with `--no-default-features`), the OCI endpoints will not be available.

### Step 1.5: Add OCI Storage to ApiContext (Plus Feature Gated)

The OCI storage needs to be accessible from the request context. Update `lib/bencher_schema/src/context/mod.rs`:

```rust
// Add to the ApiContext struct (inside #[cfg(feature = "plus")] block)
#[cfg(feature = "plus")]
pub struct ApiContext {
    // ... existing fields ...

    /// OCI Registry storage (Plus feature only)
    pub oci_storage: Option<bencher_oci::OciStorage>,
}

#[cfg(feature = "plus")]
impl ApiContext {
    // Add accessor methods
    pub fn oci_blob_store(&self) -> &bencher_oci::BlobStore {
        self.oci_storage
            .as_ref()
            .expect("OCI storage not configured")
            .blob_store()
    }

    pub fn oci_manifest_store(&self) -> &bencher_oci::ManifestStore {
        self.oci_storage
            .as_ref()
            .expect("OCI storage not configured")
            .manifest_store()
    }

    pub fn oci_upload_store(&self) -> &bencher_oci::UploadStore {
        self.oci_storage
            .as_ref()
            .expect("OCI storage not configured")
            .upload_store()
    }
}
```

**Add OCI configuration types to `lib/bencher_json/src/system/config/plus/oci.rs`:**

```rust
use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OCI Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOci {
    /// S3 storage configuration for OCI registry
    pub data_store: OciDataStore,
}

impl Sanitize for JsonOci {
    fn sanitize(&mut self) {
        self.data_store.sanitize();
    }
}

/// OCI Registry storage backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "service", rename_all = "snake_case")]
pub enum OciDataStore {
    AwsS3 {
        access_key_id: String,
        secret_access_key: Secret,
        /// S3 Access Point ARN with optional path prefix
        /// Format: arn:aws:s3:<region>:<account-id>:accesspoint/<bucket>[/oci-path]
        access_point: String,
    },
}

impl Sanitize for OciDataStore {
    fn sanitize(&mut self) {
        match self {
            Self::AwsS3 { secret_access_key, .. } => secret_access_key.sanitize(),
        }
    }
}
```

**Update `lib/bencher_json/src/system/config/plus/mod.rs` to include OCI:**

```rust
pub mod oci;

pub use oci::JsonOci;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlus {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oci: Option<JsonOci>,
}

impl Sanitize for JsonPlus {
    fn sanitize(&mut self) {
        // ... existing sanitization ...
        self.oci.sanitize();
    }
}
```

**Update `lib/bencher_config/src/config_tx.rs` to initialize OCI storage:**

```rust
// In the server initialization code (after database setup)
#[cfg(feature = "plus")]
let oci_storage = if let Some(oci_config) = &config.plus.as_ref().and_then(|p| p.oci.as_ref()) {
    Some(bencher_oci::OciStorage::try_from(oci_config.data_store.clone()).await?)
} else {
    None
};
```

**Example configuration in `bencher.json`:**

```json
{
  "plus": {
    "oci": {
      "data_store": {
        "service": "aws_s3",
        "access_key_id": "AKIAIOSFODNN7EXAMPLE",
        "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "access_point": "arn:aws:s3:us-east-1:123456789012:accesspoint/my-bucket/oci"
      }
    }
  }
}
```

This follows the exact same pattern as the existing `database.data_store` configuration for backups.

---

## Phase 2: Core Types and Storage

### Step 2.1: Define OCI Types

Create `plus/bencher_oci/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

/// OCI repository name
/// Must match: [a-z0-9]+([._-][a-z0-9]+)*(/[a-z0-9]+([._-][a-z0-9]+)*)*
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepositoryName(String);

impl RepositoryName {
    pub fn parse(name: &str) -> Result<Self, OciError> {
        // Validate against OCI spec regex
        let re = regex::Regex::new(
            r"^[a-z0-9]+([._-][a-z0-9]+)*(/[a-z0-9]+([._-][a-z0-9]+)*)*$"
        ).expect("valid regex");

        if re.is_match(name) && name.len() <= 255 {
            Ok(Self(name.to_owned()))
        } else {
            Err(OciError::InvalidRepositoryName)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// OCI content digest (e.g., sha256:abc123...)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Digest(String);

impl Digest {
    pub fn parse(digest: &str) -> Result<Self, OciError> {
        // Format: algorithm:hex
        let parts: Vec<&str> = digest.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(OciError::InvalidDigest);
        }

        let algorithm = parts[0];
        let hex_value = parts[1];

        // Validate algorithm (sha256 or sha512)
        if algorithm != "sha256" && algorithm != "sha512" {
            return Err(OciError::UnsupportedAlgorithm);
        }

        // Validate hex string
        if hex::decode(hex_value).is_err() {
            return Err(OciError::InvalidDigest);
        }

        Ok(Self(digest.to_owned()))
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        use sha2::{Sha256, Digest as _};
        let hash = Sha256::digest(data);
        Self(format!("sha256:{}", hex::encode(hash)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn algorithm(&self) -> &str {
        self.0.split(':').next().unwrap_or("sha256")
    }

    /// Get the hex portion of the digest (after the colon)
    pub fn hex(&self) -> &str {
        self.0.split(':').nth(1).unwrap_or("")
    }
}

/// OCI tag reference (max 128 chars)
/// Must match: [a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag(String);

impl Tag {
    pub fn parse(tag: &str) -> Result<Self, OciError> {
        let re = regex::Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}$")
            .expect("valid regex");

        if re.is_match(tag) {
            Ok(Self(tag.to_owned()))
        } else {
            Err(OciError::InvalidTag)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reference can be either a tag or a digest
#[derive(Debug, Clone)]
pub enum Reference {
    Tag(Tag),
    Digest(Digest),
}

impl Reference {
    pub fn parse(reference: &str) -> Result<Self, OciError> {
        if reference.contains(':') && reference.contains("sha") {
            Ok(Self::Digest(Digest::parse(reference)?))
        } else {
            Ok(Self::Tag(Tag::parse(reference)?))
        }
    }
}

/// Upload session identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UploadId(pub uuid::Uuid);

impl UploadId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for UploadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// OCI error codes per spec
#[derive(Debug, Clone, Copy)]
pub enum OciErrorCode {
    BlobUnknown,
    BlobUploadInvalid,
    BlobUploadUnknown,
    DigestInvalid,
    ManifestBlobUnknown,
    ManifestInvalid,
    ManifestUnknown,
    NameInvalid,
    NameUnknown,
    SizeInvalid,
    Unauthorized,
    Denied,
    Unsupported,
}

impl OciErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BlobUnknown => "BLOB_UNKNOWN",
            Self::BlobUploadInvalid => "BLOB_UPLOAD_INVALID",
            Self::BlobUploadUnknown => "BLOB_UPLOAD_UNKNOWN",
            Self::DigestInvalid => "DIGEST_INVALID",
            Self::ManifestBlobUnknown => "MANIFEST_BLOB_UNKNOWN",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::ManifestUnknown => "MANIFEST_UNKNOWN",
            Self::NameInvalid => "NAME_INVALID",
            Self::NameUnknown => "NAME_UNKNOWN",
            Self::SizeInvalid => "SIZE_INVALID",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Denied => "DENIED",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}
```

### Step 2.2: Define Error Types

Create `plus/bencher_oci/src/error.rs`:

```rust
use dropshot::HttpError;
use http::StatusCode;
use serde::Serialize;

use crate::types::OciErrorCode;

#[derive(Debug, thiserror::Error)]
pub enum OciError {
    #[error("Invalid repository name")]
    InvalidRepositoryName,

    #[error("Invalid digest format")]
    InvalidDigest,

    #[error("Unsupported digest algorithm")]
    UnsupportedAlgorithm,

    #[error("Invalid tag format")]
    InvalidTag,

    #[error("Blob not found")]
    BlobNotFound,

    #[error("Manifest not found")]
    ManifestNotFound,

    #[error("Upload session not found")]
    UploadNotFound,

    #[error("Digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    #[error("Referenced blob not found: {0}")]
    ReferencedBlobNotFound(String),

    #[error("Invalid manifest format: {0}")]
    InvalidManifest(String),

    #[error("Range not satisfiable")]
    RangeNotSatisfiable,

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl OciError {
    pub fn error_code(&self) -> OciErrorCode {
        match self {
            Self::InvalidRepositoryName => OciErrorCode::NameInvalid,
            Self::InvalidDigest | Self::UnsupportedAlgorithm => OciErrorCode::DigestInvalid,
            Self::InvalidTag => OciErrorCode::NameInvalid,
            Self::BlobNotFound => OciErrorCode::BlobUnknown,
            Self::ManifestNotFound => OciErrorCode::ManifestUnknown,
            Self::UploadNotFound => OciErrorCode::BlobUploadUnknown,
            Self::DigestMismatch { .. } => OciErrorCode::DigestInvalid,
            Self::ReferencedBlobNotFound(_) => OciErrorCode::ManifestBlobUnknown,
            Self::InvalidManifest(_) => OciErrorCode::ManifestInvalid,
            Self::RangeNotSatisfiable => OciErrorCode::BlobUploadInvalid,
            Self::Storage(_) | Self::Io(_) => OciErrorCode::BlobUploadInvalid,
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BlobNotFound | Self::ManifestNotFound | Self::UploadNotFound => {
                StatusCode::NOT_FOUND
            }
            Self::RangeNotSatisfiable => StatusCode::RANGE_NOT_SATISFIABLE,
            Self::InvalidRepositoryName
            | Self::InvalidDigest
            | Self::UnsupportedAlgorithm
            | Self::InvalidTag
            | Self::DigestMismatch { .. }
            | Self::ReferencedBlobNotFound(_)
            | Self::InvalidManifest(_) => StatusCode::BAD_REQUEST,
            Self::Storage(_) | Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// OCI-compliant error response format
#[derive(Debug, Serialize)]
pub struct OciErrorResponse {
    pub errors: Vec<OciErrorDetail>,
}

#[derive(Debug, Serialize)]
pub struct OciErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

impl From<OciError> for HttpError {
    fn from(err: OciError) -> Self {
        let response = OciErrorResponse {
            errors: vec![OciErrorDetail {
                code: err.error_code().as_str().to_owned(),
                message: err.to_string(),
                detail: None,
            }],
        };

        HttpError::for_status(
            Some(serde_json::to_string(&response).unwrap_or_default()),
            err.status_code(),
        )
    }
}
```

### Step 2.3: Implement S3 Storage Layer

The storage layer uses `aws-sdk-s3`, the same library already used for database backups. This ensures consistent S3 access patterns and credential handling.

Create `plus/bencher_oci/src/storage/mod.rs`:

```rust
mod blob;
mod manifest;
mod upload;

pub use blob::BlobStore;
pub use manifest::ManifestStore;
pub use upload::UploadStore;

use std::str::FromStr;
use std::sync::Arc;

use bencher_json::system::config::plus::oci::OciDataStore;
use bencher_valid::Secret;

/// S3 ARN parser (reuses the same pattern as database backup)
/// Format: arn:aws:s3:<region>:<account-id>:accesspoint/<bucket>[/path]
pub struct S3Arn {
    pub region: String,
    pub bucket_name: String,
    pub bucket_path: Option<String>,
}

impl FromStr for S3Arn {
    type Err = OciStorageError;

    fn from_str(arn: &str) -> Result<Self, Self::Err> {
        // Parse ARN following the same pattern as database.rs
        let mut parts = arn.splitn(6, ':');

        let arn_part = parts.next().ok_or(OciStorageError::InvalidArn("missing prefix".into()))?;
        if arn_part != "arn" {
            return Err(OciStorageError::InvalidArn(format!("bad prefix: {arn_part}")));
        }

        let _partition = parts.next().ok_or(OciStorageError::InvalidArn("missing partition".into()))?;
        let service = parts.next().ok_or(OciStorageError::InvalidArn("missing service".into()))?;
        if service != "s3" {
            return Err(OciStorageError::InvalidArn(format!("bad service: {service}")));
        }

        let region = parts.next().ok_or(OciStorageError::InvalidArn("missing region".into()))?.to_owned();
        let _account_id = parts.next().ok_or(OciStorageError::InvalidArn("missing account_id".into()))?;
        let resource = parts.next().ok_or(OciStorageError::InvalidArn("missing resource".into()))?;

        let (accesspoint, resource_path) = resource
            .split_once('/')
            .ok_or(OciStorageError::InvalidArn("missing accesspoint".into()))?;

        if accesspoint != "accesspoint" {
            return Err(OciStorageError::InvalidArn(format!("bad accesspoint: {accesspoint}")));
        }

        let (bucket_name, bucket_path) = if let Some((name, path)) = resource_path.split_once('/') {
            (name.to_owned(), Some(path.to_owned()))
        } else {
            (resource_path.to_owned(), None)
        };

        Ok(Self {
            region,
            bucket_name,
            bucket_path,
        })
    }
}

/// Main OCI storage container using AWS S3.
/// This is added to the ApiContext when Plus features are enabled.
pub struct OciStorage {
    client: aws_sdk_s3::Client,
    s3_arn: S3Arn,
    blob_store: Arc<BlobStore>,
    manifest_store: Arc<ManifestStore>,
    upload_store: Arc<UploadStore>,
}

#[derive(Debug, thiserror::Error)]
pub enum OciStorageError {
    #[error("Invalid S3 ARN: {0}")]
    InvalidArn(String),
    #[error("AWS S3 error: {0}")]
    AwsS3(String),
}

impl OciStorage {
    /// Create new OCI storage from configuration
    pub fn try_from_config(config: OciDataStore) -> Result<Self, OciStorageError> {
        match config {
            OciDataStore::AwsS3 {
                access_key_id,
                secret_access_key,
                access_point,
            } => Self::new_s3(access_key_id, secret_access_key, &access_point),
        }
    }

    fn new_s3(
        access_key_id: String,
        secret_access_key: Secret,
        access_point: &str,
    ) -> Result<Self, OciStorageError> {
        let credentials = aws_credential_types::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "bencher_oci",
        );
        let credentials_provider =
            aws_credential_types::provider::SharedCredentialsProvider::new(credentials);

        let s3_arn: S3Arn = access_point.parse()?;

        let config = aws_sdk_s3::Config::builder()
            .credentials_provider(credentials_provider)
            .region(aws_sdk_s3::config::Region::new(s3_arn.region.clone()))
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);

        let base_path = s3_arn.bucket_path.as_deref().unwrap_or("oci");

        Ok(Self {
            client: client.clone(),
            s3_arn,
            blob_store: Arc::new(BlobStore::new(client.clone(), base_path)),
            manifest_store: Arc::new(ManifestStore::new(client.clone(), base_path)),
            upload_store: Arc::new(UploadStore::new(client, base_path)),
        })
    }

    pub fn blob_store(&self) -> &BlobStore {
        &self.blob_store
    }

    pub fn manifest_store(&self) -> &ManifestStore {
        &self.manifest_store
    }

    pub fn upload_store(&self) -> &UploadStore {
        &self.upload_store
    }

    /// Get the S3 bucket ARN for API responses
    pub fn bucket_name(&self) -> &str {
        &self.s3_arn.bucket_name
    }
}
```

Create `plus/bencher_oci/src/storage/blob.rs`:

```rust
use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;

use crate::error::OciError;
use crate::types::{Digest, RepositoryName};

/// Blob storage backed by AWS S3.
///
/// Blobs are stored at: `<base_path>/blobs/<algorithm>/<prefix>/<digest-hex>`
/// This structure provides good distribution across S3 prefixes for performance.
pub struct BlobStore {
    client: Client,
    bucket: String,
    base_path: String,
}

impl BlobStore {
    pub fn new(client: Client, base_path: &str) -> Self {
        Self {
            client,
            bucket: String::new(), // Set from OciStorage
            base_path: base_path.to_owned(),
        }
    }

    pub fn with_bucket(mut self, bucket: &str) -> Self {
        self.bucket = bucket.to_owned();
        self
    }

    /// Get the S3 key for a blob digest
    fn blob_key(&self, digest: &Digest) -> String {
        let algorithm = digest.algorithm();
        let hex = digest.hex();
        let prefix = &hex[..2.min(hex.len())];

        format!("{}/blobs/{}/{}/{}", self.base_path, algorithm, prefix, hex)
    }

    /// Check if a blob exists
    pub async fn exists(&self, digest: &Digest) -> bool {
        let key = self.blob_key(digest);
        self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .is_ok()
    }

    /// Get blob content
    pub async fn get(&self, digest: &Digest) -> Result<Bytes, OciError> {
        let key = self.blob_key(digest);
        let result = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    OciError::BlobNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        let data = result.body.collect().await
            .map_err(|e| OciError::Storage(e.to_string()))?;

        Ok(data.into_bytes())
    }

    /// Get blob size without downloading content
    pub async fn get_size(&self, digest: &Digest) -> Result<u64, OciError> {
        let key = self.blob_key(digest);
        let result = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NotFound") {
                    OciError::BlobNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        Ok(result.content_length().unwrap_or(0) as u64)
    }

    /// Store a blob (verifies digest matches content)
    pub async fn put(&self, digest: &Digest, data: Bytes) -> Result<(), OciError> {
        // Verify digest matches content
        let computed = Digest::from_bytes(&data);
        if computed.as_str() != digest.as_str() {
            return Err(OciError::DigestMismatch {
                expected: digest.as_str().to_owned(),
                actual: computed.as_str().to_owned(),
            });
        }

        let key = self.blob_key(digest);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|e| OciError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Delete a blob
    pub async fn delete(&self, digest: &Digest) -> Result<(), OciError> {
        let key = self.blob_key(digest);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    OciError::BlobNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        Ok(())
    }

    /// Mount a blob from one repository to another.
    /// Since blobs are content-addressed and stored globally, this just
    /// verifies the blob exists.
    pub async fn mount(
        &self,
        _from: &RepositoryName,
        _to: &RepositoryName,
        digest: &Digest,
    ) -> Result<bool, OciError> {
        Ok(self.exists(digest).await)
    }
}
```

Create `plus/bencher_oci/src/storage/upload.rs`:

```rust
use aws_sdk_s3::Client;
use bytes::{Bytes, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::OciError;
use crate::types::{Digest, RepositoryName, UploadId};

/// In-progress upload session tracking
#[derive(Debug)]
struct UploadSession {
    pub id: UploadId,
    pub repository: RepositoryName,
    /// Current upload offset (total bytes received)
    pub offset: u64,
    /// S3 multipart upload ID (for large uploads)
    pub multipart_id: Option<String>,
    /// Accumulated data for small uploads
    pub buffer: BytesMut,
}

/// Upload store using S3.
///
/// Upload strategy:
/// - Small uploads (<5MB): Buffer in memory, upload on complete
/// - Large uploads (>=5MB): Use S3 multipart upload (future enhancement)
pub struct UploadStore {
    client: Client,
    bucket: String,
    base_path: String,
    sessions: Arc<RwLock<HashMap<UploadId, UploadSession>>>,
}

impl UploadStore {
    pub fn new(client: Client, base_path: &str) -> Self {
        Self {
            client,
            bucket: String::new(),
            base_path: base_path.to_owned(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_bucket(mut self, bucket: &str) -> Self {
        self.bucket = bucket.to_owned();
        self
    }

    /// Create a new upload session
    pub async fn create(&self, repository: RepositoryName) -> Result<UploadId, OciError> {
        let id = UploadId::new();

        let session = UploadSession {
            id: id.clone(),
            repository,
            offset: 0,
            multipart_id: None,
            buffer: BytesMut::new(),
        };

        self.sessions.write().await.insert(id.clone(), session);

        Ok(id)
    }

    /// Get current upload session offset
    pub async fn get_session(&self, id: &UploadId) -> Result<u64, OciError> {
        let sessions = self.sessions.read().await;
        sessions
            .get(id)
            .map(|s| s.offset)
            .ok_or(OciError::UploadNotFound)
    }

    /// Append data to an upload session
    pub async fn append(
        &self,
        id: &UploadId,
        data: &[u8],
        start: Option<u64>,
    ) -> Result<u64, OciError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(id).ok_or(OciError::UploadNotFound)?;

        // Validate range if provided (chunked upload)
        if let Some(start_offset) = start {
            if start_offset != session.offset {
                return Err(OciError::RangeNotSatisfiable);
            }
        }

        // Buffer data in memory
        // For production with large images, implement S3 multipart upload
        session.buffer.extend_from_slice(data);
        session.offset += data.len() as u64;

        Ok(session.offset)
    }

    /// Complete an upload and return the data with computed digest
    pub async fn complete(
        &self,
        id: &UploadId,
        final_chunk: Option<&[u8]>,
    ) -> Result<(Bytes, Digest), OciError> {
        // Append final chunk if provided
        if let Some(data) = final_chunk {
            self.append(id, data, None).await?;
        }

        let mut sessions = self.sessions.write().await;
        let session = sessions.remove(id).ok_or(OciError::UploadNotFound)?;

        let data = session.buffer.freeze();
        let digest = Digest::from_bytes(&data);

        Ok((data, digest))
    }

    /// Cancel an upload session
    pub async fn cancel(&self, id: &UploadId) -> Result<(), OciError> {
        let mut sessions = self.sessions.write().await;
        let _session = sessions.remove(id).ok_or(OciError::UploadNotFound)?;

        // For multipart uploads, would abort here:
        // self.client.abort_multipart_upload()...

        Ok(())
    }
}
```

Create `plus/bencher_oci/src/storage/manifest.rs`:

```rust
use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::OciError;
use crate::types::{Digest, RepositoryName, Tag};

/// Stored manifest with metadata
#[derive(Debug, Clone)]
pub struct StoredManifest {
    pub content: Bytes,
    pub content_type: String,
    pub digest: Digest,
}

/// Manifest metadata stored alongside the manifest content
#[derive(Debug, Serialize, Deserialize)]
struct ManifestMeta {
    content_type: String,
    digest: String,
}

/// Manifest storage backed by AWS S3.
///
/// Storage layout:
/// - `<base_path>/manifests/<repo>/<digest>.json` - manifest content
/// - `<base_path>/manifests/<repo>/<digest>.meta` - manifest metadata
/// - `<base_path>/manifests/<repo>/_tags/<tag>` - tag to digest mapping
pub struct ManifestStore {
    client: Client,
    bucket: String,
    base_path: String,
    /// In-memory tag cache
    tag_cache: Arc<RwLock<HashMap<String, HashMap<String, Digest>>>>,
}

impl ManifestStore {
    pub fn new(client: Client, base_path: &str) -> Self {
        Self {
            client,
            bucket: String::new(),
            base_path: base_path.to_owned(),
            tag_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_bucket(mut self, bucket: &str) -> Self {
        self.bucket = bucket.to_owned();
        self
    }

    fn repo_prefix(&self, repo: &RepositoryName) -> String {
        format!("{}/manifests/{}", self.base_path, repo.as_str().replace('/', "_"))
    }

    fn manifest_key(&self, repo: &RepositoryName, digest: &Digest) -> String {
        format!("{}/{}.json", self.repo_prefix(repo), digest.hex())
    }

    fn meta_key(&self, repo: &RepositoryName, digest: &Digest) -> String {
        format!("{}/{}.meta", self.repo_prefix(repo), digest.hex())
    }

    fn tag_key(&self, repo: &RepositoryName, tag: &Tag) -> String {
        format!("{}/_tags/{}", self.repo_prefix(repo), tag.as_str())
    }

    /// Check if a manifest exists
    pub async fn exists(&self, repo: &RepositoryName, digest: &Digest) -> bool {
        let key = self.manifest_key(repo, digest);
        self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .is_ok()
    }

    /// Get a manifest by digest
    pub async fn get(
        &self,
        repo: &RepositoryName,
        digest: &Digest,
    ) -> Result<StoredManifest, OciError> {
        let key = self.manifest_key(repo, digest);
        let result = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    OciError::ManifestNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        let content = result.body.collect().await
            .map_err(|e| OciError::Storage(e.to_string()))?
            .into_bytes();

        // Get metadata
        let meta_key = self.meta_key(repo, digest);
        let content_type = match self.client.get_object().bucket(&self.bucket).key(&meta_key).send().await {
            Ok(result) => {
                let meta_bytes = result.body.collect().await
                    .map_err(|e| OciError::Storage(e.to_string()))?
                    .into_bytes();
                let meta: ManifestMeta = serde_json::from_slice(&meta_bytes)
                    .unwrap_or(ManifestMeta {
                        content_type: "application/vnd.oci.image.manifest.v1+json".to_owned(),
                        digest: digest.as_str().to_owned(),
                    });
                meta.content_type
            }
            Err(_) => "application/vnd.oci.image.manifest.v1+json".to_owned(),
        };

        Ok(StoredManifest {
            content,
            content_type,
            digest: digest.clone(),
        })
    }

    /// Get a manifest by tag
    pub async fn get_by_tag(
        &self,
        repo: &RepositoryName,
        tag: &Tag,
    ) -> Result<StoredManifest, OciError> {
        let tag_key = self.tag_key(repo, tag);

        let result = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&tag_key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    OciError::ManifestNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        let digest_bytes = result.body.collect().await
            .map_err(|e| OciError::Storage(e.to_string()))?
            .into_bytes();
        let digest_str = String::from_utf8_lossy(&digest_bytes);
        let digest = Digest::parse(&digest_str)?;

        self.get(repo, &digest).await
    }

    /// Store a manifest
    pub async fn put(
        &self,
        repo: &RepositoryName,
        tag: Option<&Tag>,
        content: &[u8],
        content_type: &str,
    ) -> Result<Digest, OciError> {
        let digest = Digest::from_bytes(content);

        // Store manifest content
        let key = self.manifest_key(repo, &digest);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::copy_from_slice(content)))
            .send()
            .await
            .map_err(|e| OciError::Storage(e.to_string()))?;

        // Store metadata
        let meta = ManifestMeta {
            content_type: content_type.to_owned(),
            digest: digest.as_str().to_owned(),
        };
        let meta_bytes = serde_json::to_vec(&meta).map_err(|e| OciError::Storage(e.to_string()))?;
        let meta_key = self.meta_key(repo, &digest);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&meta_key)
            .body(ByteStream::from(Bytes::from(meta_bytes)))
            .send()
            .await
            .map_err(|e| OciError::Storage(e.to_string()))?;

        // Store tag mapping if provided
        if let Some(tag) = tag {
            let tag_key = self.tag_key(repo, tag);
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&tag_key)
                .body(ByteStream::from(Bytes::from(digest.as_str().to_owned())))
                .send()
                .await
                .map_err(|e| OciError::Storage(e.to_string()))?;

            // Update cache
            let mut cache = self.tag_cache.write().await;
            cache
                .entry(repo.as_str().to_owned())
                .or_default()
                .insert(tag.as_str().to_owned(), digest.clone());
        }

        Ok(digest)
    }

    /// List all tags for a repository
    pub async fn list_tags(&self, repo: &RepositoryName) -> Result<Vec<String>, OciError> {
        let prefix = format!("{}/_tags/", self.repo_prefix(repo));

        let result = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| OciError::Storage(e.to_string()))?;

        let mut tags: Vec<String> = result
            .contents()
            .iter()
            .filter_map(|obj| {
                obj.key()
                    .and_then(|k| k.strip_prefix(&prefix))
                    .map(|s| s.to_owned())
            })
            .collect();

        tags.sort();
        Ok(tags)
    }

    /// Delete a manifest by digest
    pub async fn delete(&self, repo: &RepositoryName, digest: &Digest) -> Result<(), OciError> {
        let key = self.manifest_key(repo, digest);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    OciError::ManifestNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        // Also delete metadata
        let meta_key = self.meta_key(repo, digest);
        let _ = self.client.delete_object().bucket(&self.bucket).key(&meta_key).send().await;

        // Clear cache entries pointing to this digest
        let mut cache = self.tag_cache.write().await;
        if let Some(tags) = cache.get_mut(repo.as_str()) {
            tags.retain(|_, d| d.as_str() != digest.as_str());
        }

        Ok(())
    }

    /// Delete a tag (does not delete the manifest)
    pub async fn delete_tag(&self, repo: &RepositoryName, tag: &Tag) -> Result<(), OciError> {
        let tag_key = self.tag_key(repo, tag);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&tag_key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") {
                    OciError::ManifestNotFound
                } else {
                    OciError::Storage(e.to_string())
                }
            })?;

        // Clear from cache
        let mut cache = self.tag_cache.write().await;
        if let Some(tags) = cache.get_mut(repo.as_str()) {
            tags.remove(tag.as_str());
        }

        Ok(())
    }
}
```

---

## Phase 3: Base Endpoint

### Step 3.1: Create the /v2/ Endpoint

This endpoint is required for all OCI registries. It returns 200 OK to indicate v2 API support.

Create `plus/bencher_oci/src/endpoints/mod.rs`:

```rust
mod base;
mod blobs;
mod manifests;
mod tags;
mod referrers;

use dropshot::ApiDescription;

pub fn register(
    api: &mut ApiDescription<bencher_schema::ApiContext>,
    http_options: bool,
) -> Result<(), dropshot::ApiDescriptionRegisterError> {
    // Base endpoint
    if http_options {
        api.register(base::v2_options)?;
    }
    api.register(base::v2_get)?;

    // Blob endpoints
    if http_options {
        api.register(blobs::blob_head_options)?;
        api.register(blobs::blob_get_options)?;
        api.register(blobs::blob_delete_options)?;
        api.register(blobs::upload_post_options)?;
        api.register(blobs::upload_patch_options)?;
        api.register(blobs::upload_put_options)?;
        api.register(blobs::upload_get_options)?;
        api.register(blobs::upload_delete_options)?;
    }
    api.register(blobs::blob_head)?;
    api.register(blobs::blob_get)?;
    api.register(blobs::blob_delete)?;
    api.register(blobs::upload_post)?;
    api.register(blobs::upload_patch)?;
    api.register(blobs::upload_put)?;
    api.register(blobs::upload_get)?;
    api.register(blobs::upload_delete)?;

    // Manifest endpoints
    if http_options {
        api.register(manifests::manifest_head_options)?;
        api.register(manifests::manifest_get_options)?;
        api.register(manifests::manifest_put_options)?;
        api.register(manifests::manifest_delete_options)?;
    }
    api.register(manifests::manifest_head)?;
    api.register(manifests::manifest_get)?;
    api.register(manifests::manifest_put)?;
    api.register(manifests::manifest_delete)?;

    // Tags endpoint
    if http_options {
        api.register(tags::tags_list_options)?;
    }
    api.register(tags::tags_list)?;

    // Referrers endpoint
    if http_options {
        api.register(referrers::referrers_get_options)?;
    }
    api.register(referrers::referrers_get)?;

    Ok(())
}
```

Create `plus/bencher_oci/src/endpoints/base.rs`:

```rust
use dropshot::{endpoint, HttpError, RequestContext};
use http::{Response, StatusCode};
use hyper::Body;

use bencher_schema::ApiContext;

/// OPTIONS /v2/
#[endpoint {
    method = OPTIONS,
    path = "/v2/",
    tags = ["oci"]
}]
pub async fn v2_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}

/// GET /v2/
///
/// Check that the endpoint implements OCI Distribution Specification.
/// Returns 200 OK if the registry implements the V2 API.
#[endpoint {
    method = GET,
    path = "/v2/",
    tags = ["oci"]
}]
pub async fn v2_get(
    _rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    // Emit OTEL metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::V2Check);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}
```

---

## Phase 4: Blob Push Endpoints

### Step 4.1: Implement Blob Upload Initiation

Create `plus/bencher_oci/src/endpoints/blobs.rs`:

```rust
use dropshot::{
    endpoint, HttpError, Path, Query, RequestContext, TypedBody,
    UntypedBody,
};
use http::{Response, StatusCode};
use hyper::Body;
use schemars::JsonSchema;
use serde::Deserialize;

use bencher_schema::ApiContext;
use crate::error::OciError;
use crate::storage::{BlobStore, UploadStore};
use crate::types::{Digest, RepositoryName, UploadId};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepositoryPath {
    /// Repository name (e.g., "library/alpine")
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlobPath {
    /// Repository name
    name: String,
    /// Content digest (e.g., "sha256:abc123...")
    digest: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadPath {
    /// Repository name
    name: String,
    /// Upload session UUID
    session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadQuery {
    /// Digest for completing upload
    digest: Option<String>,
    /// Source repository for cross-mount
    from: Option<String>,
}

/// HEAD /v2/{name}/blobs/{digest}
///
/// Check if a blob exists.
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci", "blobs"]
}]
pub async fn blob_head(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let _repo = RepositoryName::parse(&path.name)?;
    let digest = Digest::parse(&path.digest)?;

    let context = rqctx.context();
    let blob_store = context.oci_blob_store();

    if !blob_store.exists(&digest).await {
        return Err(OciError::BlobNotFound.into());
    }

    let size = blob_store.get_size(&digest).await?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobHead);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Length", size)
        .header("Docker-Content-Digest", digest.as_str())
        .body(Body::empty())
        .unwrap())
}

/// GET /v2/{name}/blobs/{digest}
///
/// Retrieve a blob by digest.
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci", "blobs"]
}]
pub async fn blob_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let _repo = RepositoryName::parse(&path.name)?;
    let digest = Digest::parse(&path.digest)?;

    let context = rqctx.context();
    let blob_store = context.oci_blob_store();

    let data = blob_store.get(&digest).await?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobGet);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Length", data.len())
        .header("Content-Type", "application/octet-stream")
        .header("Docker-Content-Digest", digest.as_str())
        .body(Body::from(data))
        .unwrap())
}

/// DELETE /v2/{name}/blobs/{digest}
///
/// Delete a blob.
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci", "blobs"]
}]
pub async fn blob_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let _repo = RepositoryName::parse(&path.name)?;
    let digest = Digest::parse(&path.digest)?;

    let context = rqctx.context();
    let blob_store = context.oci_blob_store();

    blob_store.delete(&digest).await?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobDelete);

    Ok(Response::builder()
        .status(StatusCode::ACCEPTED)
        .body(Body::empty())
        .unwrap())
}

/// POST /v2/{name}/blobs/uploads/
///
/// Initiate a blob upload session.
/// Supports:
/// - Monolithic upload with ?digest=<digest>
/// - Cross-repository mount with ?digest=<digest>&from=<repo>
/// - Session-based upload (returns Location header)
#[endpoint {
    method = POST,
    path = "/v2/{name}/blobs/uploads/",
    tags = ["oci", "blobs"]
}]
pub async fn upload_post(
    rqctx: RequestContext<ApiContext>,
    path: Path<RepositoryPath>,
    query: Query<UploadQuery>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();
    let repo = RepositoryName::parse(&path.name)?;

    let context = rqctx.context();
    let blob_store = context.oci_blob_store();
    let upload_store = context.oci_upload_store();

    // Handle monolithic upload with digest
    if let Some(digest_str) = &query.digest {
        let digest = Digest::parse(digest_str)?;

        // Handle cross-mount
        if let Some(from_repo) = &query.from {
            let from_repo = RepositoryName::parse(from_repo)?;
            if blob_store.mount(&from_repo, &repo, &digest).await? {
                let location = format!("/v2/{}/blobs/{}", repo.as_str(), digest.as_str());

                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(
                    bencher_otel::oci::OciCounter::BlobMount
                );

                return Ok(Response::builder()
                    .status(StatusCode::CREATED)
                    .header("Location", location)
                    .header("Docker-Content-Digest", digest.as_str())
                    .body(Body::empty())
                    .unwrap());
            }
        }

        // Monolithic upload
        let data = body.as_bytes();
        if !data.is_empty() {
            blob_store.put(&digest, data).await?;

            let location = format!("/v2/{}/blobs/{}", repo.as_str(), digest.as_str());

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(
                bencher_otel::oci::OciCounter::BlobUploadMonolithic
            );

            return Ok(Response::builder()
                .status(StatusCode::CREATED)
                .header("Location", location)
                .header("Docker-Content-Digest", digest.as_str())
                .body(Body::empty())
                .unwrap());
        }
    }

    // Start upload session
    let upload_id = upload_store.create(repo.clone()).await?;
    let location = format!(
        "/v2/{}/blobs/uploads/{}",
        repo.as_str(),
        upload_id
    );

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobUploadStart);

    Ok(Response::builder()
        .status(StatusCode::ACCEPTED)
        .header("Location", location)
        .header("Range", "0-0")
        .header("Docker-Upload-UUID", upload_id.to_string())
        .body(Body::empty())
        .unwrap())
}

/// PATCH /v2/{name}/blobs/uploads/{session_id}
///
/// Upload a chunk of blob data.
#[endpoint {
    method = PATCH,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci", "blobs"]
}]
pub async fn upload_patch(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let upload_id = UploadId(
        path.session_id
            .parse()
            .map_err(|_| OciError::UploadNotFound)?
    );

    let context = rqctx.context();
    let upload_store = context.oci_upload_store();

    // Get Content-Range header for offset validation
    // Format: start-end (e.g., "0-1023")
    let content_range = rqctx
        .request
        .headers()
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            let parts: Vec<&str> = s.split('-').collect();
            parts.first().and_then(|start| start.parse::<u64>().ok())
        });

    let data = body.as_bytes();
    let new_offset = upload_store.append(&upload_id, data, content_range).await?;

    let location = format!(
        "/v2/{}/blobs/uploads/{}",
        repo.as_str(),
        upload_id
    );

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobUploadChunk);

    Ok(Response::builder()
        .status(StatusCode::ACCEPTED)
        .header("Location", location)
        .header("Range", format!("0-{}", new_offset.saturating_sub(1)))
        .header("Docker-Upload-UUID", upload_id.to_string())
        .body(Body::empty())
        .unwrap())
}

/// PUT /v2/{name}/blobs/uploads/{session_id}
///
/// Complete a blob upload.
#[endpoint {
    method = PUT,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci", "blobs"]
}]
pub async fn upload_put(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
    query: Query<UploadQuery>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let upload_id = UploadId(
        path.session_id
            .parse()
            .map_err(|_| OciError::UploadNotFound)?
    );

    let expected_digest = query
        .digest
        .as_ref()
        .map(|d| Digest::parse(d))
        .transpose()?
        .ok_or_else(|| OciError::InvalidDigest)?;

    let context = rqctx.context();
    let blob_store = context.oci_blob_store();
    let upload_store = context.oci_upload_store();

    // Complete upload with optional final chunk
    let final_chunk = body.as_bytes();
    let final_chunk = if final_chunk.is_empty() {
        None
    } else {
        Some(final_chunk)
    };

    let (data, computed_digest) = upload_store.complete(&upload_id, final_chunk).await?;

    // Verify digest
    if computed_digest.as_str() != expected_digest.as_str() {
        return Err(OciError::DigestMismatch {
            expected: expected_digest.as_str().to_owned(),
            actual: computed_digest.as_str().to_owned(),
        }
        .into());
    }

    // Store the blob
    blob_store.put(&computed_digest, &data).await?;

    let location = format!("/v2/{}/blobs/{}", repo.as_str(), computed_digest.as_str());

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobUploadComplete);

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .header("Location", location)
        .header("Docker-Content-Digest", computed_digest.as_str())
        .body(Body::empty())
        .unwrap())
}

/// GET /v2/{name}/blobs/uploads/{session_id}
///
/// Get upload session status.
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci", "blobs"]
}]
pub async fn upload_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let upload_id = UploadId(
        path.session_id
            .parse()
            .map_err(|_| OciError::UploadNotFound)?
    );

    let context = rqctx.context();
    let upload_store = context.oci_upload_store();

    let offset = upload_store.get_session(&upload_id).await?;

    let location = format!(
        "/v2/{}/blobs/uploads/{}",
        repo.as_str(),
        upload_id
    );

    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Location", location)
        .header("Range", format!("0-{}", offset.saturating_sub(1)))
        .header("Docker-Upload-UUID", upload_id.to_string())
        .body(Body::empty())
        .unwrap())
}

/// DELETE /v2/{name}/blobs/uploads/{session_id}
///
/// Cancel an upload session.
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci", "blobs"]
}]
pub async fn upload_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let _repo = RepositoryName::parse(&path.name)?;
    let upload_id = UploadId(
        path.session_id
            .parse()
            .map_err(|_| OciError::UploadNotFound)?
    );

    let context = rqctx.context();
    let upload_store = context.oci_upload_store();

    upload_store.cancel(&upload_id).await?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobUploadCancel);

    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap())
}

// OPTIONS endpoints (implement similarly for CORS)
#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/{digest}", tags = ["oci"] }]
pub async fn blob_head_options(_: RequestContext<ApiContext>, _: Path<BlobPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/{digest}", tags = ["oci"] }]
pub async fn blob_get_options(_: RequestContext<ApiContext>, _: Path<BlobPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/{digest}", tags = ["oci"] }]
pub async fn blob_delete_options(_: RequestContext<ApiContext>, _: Path<BlobPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/uploads/", tags = ["oci"] }]
pub async fn upload_post_options(_: RequestContext<ApiContext>, _: Path<RepositoryPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/uploads/{session_id}", tags = ["oci"] }]
pub async fn upload_patch_options(_: RequestContext<ApiContext>, _: Path<UploadPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/uploads/{session_id}", tags = ["oci"] }]
pub async fn upload_put_options(_: RequestContext<ApiContext>, _: Path<UploadPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/uploads/{session_id}", tags = ["oci"] }]
pub async fn upload_get_options(_: RequestContext<ApiContext>, _: Path<UploadPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/blobs/uploads/{session_id}", tags = ["oci"] }]
pub async fn upload_delete_options(_: RequestContext<ApiContext>, _: Path<UploadPath>) -> Result<Response<Body>, HttpError> {
    options_response()
}

fn options_response() -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}
```

---

## Phase 5: Manifest Push Endpoint

### Step 5.1: Implement Manifest Upload

Create `plus/bencher_oci/src/endpoints/manifests.rs`:

```rust
use dropshot::{endpoint, HttpError, Path, RequestContext, UntypedBody};
use http::{Response, StatusCode};
use hyper::Body;
use schemars::JsonSchema;
use serde::Deserialize;

use bencher_schema::ApiContext;
use crate::error::OciError;
use crate::types::{Digest, Reference, RepositoryName};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManifestPath {
    /// Repository name
    name: String,
    /// Tag or digest reference
    reference: String,
}

/// HEAD /v2/{name}/manifests/{reference}
///
/// Check if a manifest exists.
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci", "manifests"]
}]
pub async fn manifest_head(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let reference = Reference::parse(&path.reference)?;

    let context = rqctx.context();
    let manifest_store = context.oci_manifest_store();

    let manifest = match reference {
        Reference::Tag(tag) => manifest_store.get_by_tag(&repo, &tag).await?,
        Reference::Digest(digest) => manifest_store.get(&repo, &digest).await?,
    };

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::ManifestHead);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", &manifest.content_type)
        .header("Content-Length", manifest.content.len())
        .header("Docker-Content-Digest", manifest.digest.as_str())
        .body(Body::empty())
        .unwrap())
}

/// GET /v2/{name}/manifests/{reference}
///
/// Retrieve a manifest by tag or digest.
#[endpoint {
    method = GET,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci", "manifests"]
}]
pub async fn manifest_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let reference = Reference::parse(&path.reference)?;

    let context = rqctx.context();
    let manifest_store = context.oci_manifest_store();

    let manifest = match reference {
        Reference::Tag(tag) => manifest_store.get_by_tag(&repo, &tag).await?,
        Reference::Digest(digest) => manifest_store.get(&repo, &digest).await?,
    };

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::ManifestGet);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", &manifest.content_type)
        .header("Content-Length", manifest.content.len())
        .header("Docker-Content-Digest", manifest.digest.as_str())
        .body(Body::from(manifest.content))
        .unwrap())
}

/// PUT /v2/{name}/manifests/{reference}
///
/// Upload a manifest.
#[endpoint {
    method = PUT,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci", "manifests"]
}]
pub async fn manifest_put(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let reference = Reference::parse(&path.reference)?;

    // Get content type from request
    let content_type = rqctx
        .request
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/vnd.oci.image.manifest.v1+json");

    let content = body.as_bytes();

    // Parse and validate manifest structure
    let manifest_json: serde_json::Value = serde_json::from_slice(content)
        .map_err(|e| OciError::InvalidManifest(e.to_string()))?;

    // Validate referenced blobs exist (unless it's an index)
    let context = rqctx.context();
    let blob_store = context.oci_blob_store();

    // Check config blob if present
    if let Some(config) = manifest_json.get("config") {
        if let Some(digest_str) = config.get("digest").and_then(|d| d.as_str()) {
            let digest = Digest::parse(digest_str)?;
            if !blob_store.exists(&digest).await {
                return Err(OciError::ReferencedBlobNotFound(digest_str.to_owned()).into());
            }
        }
    }

    // Check layer blobs if present
    if let Some(layers) = manifest_json.get("layers").and_then(|l| l.as_array()) {
        for layer in layers {
            if let Some(digest_str) = layer.get("digest").and_then(|d| d.as_str()) {
                let digest = Digest::parse(digest_str)?;
                if !blob_store.exists(&digest).await {
                    return Err(OciError::ReferencedBlobNotFound(digest_str.to_owned()).into());
                }
            }
        }
    }

    // Store manifest
    let manifest_store = context.oci_manifest_store();
    let tag = match &reference {
        Reference::Tag(t) => Some(t),
        Reference::Digest(_) => None,
    };

    let digest = manifest_store.put(&repo, tag, content, content_type).await?;

    let location = format!("/v2/{}/manifests/{}", repo.as_str(), digest.as_str());

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::ManifestPut);

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .header("Location", location)
        .header("Docker-Content-Digest", digest.as_str())
        .body(Body::empty())
        .unwrap())
}

/// DELETE /v2/{name}/manifests/{reference}
///
/// Delete a manifest by digest.
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci", "manifests"]
}]
pub async fn manifest_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repo = RepositoryName::parse(&path.name)?;
    let reference = Reference::parse(&path.reference)?;

    let context = rqctx.context();
    let manifest_store = context.oci_manifest_store();

    match reference {
        Reference::Digest(digest) => {
            manifest_store.delete(&repo, &digest).await?;
        }
        Reference::Tag(tag) => {
            // Per spec, DELETE by tag should return error or delete the tag
            manifest_store.delete_tag(&repo, &tag).await?;
        }
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::ManifestDelete);

    Ok(Response::builder()
        .status(StatusCode::ACCEPTED)
        .body(Body::empty())
        .unwrap())
}

// OPTIONS endpoints
#[endpoint { method = OPTIONS, path = "/v2/{name}/manifests/{reference}", tags = ["oci"] }]
pub async fn manifest_head_options(_: RequestContext<ApiContext>, _: Path<ManifestPath>) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/manifests/{reference}", tags = ["oci"] }]
pub async fn manifest_get_options(_: RequestContext<ApiContext>, _: Path<ManifestPath>) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/manifests/{reference}", tags = ["oci"] }]
pub async fn manifest_put_options(_: RequestContext<ApiContext>, _: Path<ManifestPath>) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/manifests/{reference}", tags = ["oci"] }]
pub async fn manifest_delete_options(_: RequestContext<ApiContext>, _: Path<ManifestPath>) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}
```

---

## Phase 6: Pull Endpoints

Pull endpoints were included in Phases 4 and 5:

- `GET /v2/{name}/blobs/{digest}` - Retrieve blob
- `HEAD /v2/{name}/blobs/{digest}` - Check blob exists
- `GET /v2/{name}/manifests/{reference}` - Retrieve manifest
- `HEAD /v2/{name}/manifests/{reference}` - Check manifest exists

These are **required** for OCI conformance.

---

## Phase 7: Content Discovery

### Step 7.1: Implement Tag Listing

Create `plus/bencher_oci/src/endpoints/tags.rs`:

```rust
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::{Response, StatusCode};
use hyper::Body;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use bencher_schema::ApiContext;
use crate::types::RepositoryName;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsPath {
    /// Repository name
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsQuery {
    /// Maximum number of results
    n: Option<usize>,
    /// Pagination marker (last tag from previous request)
    last: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TagList {
    name: String,
    tags: Vec<String>,
}

/// GET /v2/{name}/tags/list
///
/// List all tags for a repository.
#[endpoint {
    method = GET,
    path = "/v2/{name}/tags/list",
    tags = ["oci", "tags"]
}]
pub async fn tags_list(
    rqctx: RequestContext<ApiContext>,
    path: Path<TagsPath>,
    query: Query<TagsQuery>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();
    let repo = RepositoryName::parse(&path.name)?;

    let context = rqctx.context();
    let manifest_store = context.oci_manifest_store();

    let mut tags = manifest_store.list_tags(&repo).await?;
    tags.sort();

    // Apply pagination
    let tags: Vec<String> = if let Some(last) = &query.last {
        tags.into_iter()
            .skip_while(|t| t <= last)
            .take(query.n.unwrap_or(usize::MAX))
            .collect()
    } else {
        tags.into_iter()
            .take(query.n.unwrap_or(usize::MAX))
            .collect()
    };

    let response = TagList {
        name: repo.as_str().to_owned(),
        tags: tags.clone(),
    };

    // Build response with optional Link header for pagination
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json");

    if let Some(n) = query.n {
        if tags.len() == n {
            if let Some(last_tag) = tags.last() {
                let link = format!(
                    "</v2/{}/tags/list?n={}&last={}>; rel=\"next\"",
                    repo.as_str(),
                    n,
                    last_tag
                );
                builder = builder.header("Link", link);
            }
        }
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::TagsList);

    Ok(builder
        .body(Body::from(serde_json::to_vec(&response).unwrap()))
        .unwrap())
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/tags/list", tags = ["oci"] }]
pub async fn tags_list_options(
    _: RequestContext<ApiContext>,
    _: Path<TagsPath>,
) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}
```

### Step 7.2: Implement Referrers Endpoint

Create `plus/bencher_oci/src/endpoints/referrers.rs`:

```rust
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::{Response, StatusCode};
use hyper::Body;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use bencher_schema::ApiContext;
use crate::types::{Digest, RepositoryName};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReferrersPath {
    /// Repository name
    name: String,
    /// Subject manifest digest
    digest: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReferrersQuery {
    /// Filter by artifact type
    #[serde(rename = "artifactType")]
    artifact_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferrersResponse {
    schema_version: u32,
    media_type: String,
    manifests: Vec<ReferrerDescriptor>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferrerDescriptor {
    media_type: String,
    digest: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<serde_json::Value>,
}

/// GET /v2/{name}/referrers/{digest}
///
/// List manifests that reference the specified digest via the "subject" field.
#[endpoint {
    method = GET,
    path = "/v2/{name}/referrers/{digest}",
    tags = ["oci", "referrers"]
}]
pub async fn referrers_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<ReferrersPath>,
    query: Query<ReferrersQuery>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();
    let _repo = RepositoryName::parse(&path.name)?;
    let _digest = Digest::parse(&path.digest)?;

    // TODO: Implement referrer tracking
    // For now, return empty list (valid per spec)
    let response = ReferrersResponse {
        schema_version: 2,
        media_type: "application/vnd.oci.image.index.v1+json".to_owned(),
        manifests: vec![],
    };

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.oci.image.index.v1+json");

    // Add filter header if filter was applied
    if let Some(filter) = &query.artifact_type {
        builder = builder.header("OCI-Filters-Applied", format!("artifactType={}", filter));
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::ReferrersGet);

    Ok(builder
        .body(Body::from(serde_json::to_vec(&response).unwrap()))
        .unwrap())
}

#[endpoint { method = OPTIONS, path = "/v2/{name}/referrers/{digest}", tags = ["oci"] }]
pub async fn referrers_get_options(
    _: RequestContext<ApiContext>,
    _: Path<ReferrersPath>,
) -> Result<Response<Body>, HttpError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Docker-Distribution-API-Version", "registry/2.0")
        .body(Body::empty())
        .unwrap())
}
```

---

## Phase 8: Content Management

Content management endpoints were implemented in earlier phases:

- `DELETE /v2/{name}/blobs/{digest}` - Delete blob (Phase 4)
- `DELETE /v2/{name}/manifests/{digest}` - Delete manifest (Phase 5)

---

## Phase 9: OTEL Metrics

### Step 9.1: Add OCI Metrics to bencher_otel

Update `plus/bencher_otel/src/lib.rs` to add OCI module:

```rust
pub mod oci;
```

Create `plus/bencher_otel/src/oci.rs`:

```rust
use crate::ApiMeter;
use core::fmt;

/// OCI-specific metrics
#[derive(Debug, Clone, Copy)]
pub enum OciCounter {
    // Base
    V2Check,

    // Blob operations
    BlobHead,
    BlobGet,
    BlobDelete,
    BlobMount,

    // Blob uploads
    BlobUploadStart,
    BlobUploadChunk,
    BlobUploadComplete,
    BlobUploadCancel,
    BlobUploadMonolithic,

    // Manifest operations
    ManifestHead,
    ManifestGet,
    ManifestPut,
    ManifestDelete,

    // Content discovery
    TagsList,
    ReferrersGet,
}

impl OciCounter {
    pub fn name(&self) -> &'static str {
        match self {
            Self::V2Check => "oci.v2.check",

            Self::BlobHead => "oci.blob.head",
            Self::BlobGet => "oci.blob.get",
            Self::BlobDelete => "oci.blob.delete",
            Self::BlobMount => "oci.blob.mount",

            Self::BlobUploadStart => "oci.blob.upload.start",
            Self::BlobUploadChunk => "oci.blob.upload.chunk",
            Self::BlobUploadComplete => "oci.blob.upload.complete",
            Self::BlobUploadCancel => "oci.blob.upload.cancel",
            Self::BlobUploadMonolithic => "oci.blob.upload.monolithic",

            Self::ManifestHead => "oci.manifest.head",
            Self::ManifestGet => "oci.manifest.get",
            Self::ManifestPut => "oci.manifest.put",
            Self::ManifestDelete => "oci.manifest.delete",

            Self::TagsList => "oci.tags.list",
            Self::ReferrersGet => "oci.referrers.get",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::V2Check => "OCI API version checks",

            Self::BlobHead => "Blob existence checks",
            Self::BlobGet => "Blob downloads",
            Self::BlobDelete => "Blob deletions",
            Self::BlobMount => "Cross-repository blob mounts",

            Self::BlobUploadStart => "Blob upload sessions started",
            Self::BlobUploadChunk => "Blob upload chunks received",
            Self::BlobUploadComplete => "Blob uploads completed",
            Self::BlobUploadCancel => "Blob uploads cancelled",
            Self::BlobUploadMonolithic => "Monolithic blob uploads",

            Self::ManifestHead => "Manifest existence checks",
            Self::ManifestGet => "Manifest downloads",
            Self::ManifestPut => "Manifest uploads",
            Self::ManifestDelete => "Manifest deletions",

            Self::TagsList => "Tag list requests",
            Self::ReferrersGet => "Referrer list requests",
        }
    }
}

impl ApiMeter {
    pub fn increment_oci(counter: OciCounter) {
        let meter = Self::new();
        let c = meter
            .meter
            .u64_counter(counter.name().to_owned())
            .with_description(counter.description().to_owned())
            .build();
        c.add(1, &[]);
    }
}
```

### Step 9.2: Use Metrics in Endpoints

Metrics are already integrated in the endpoint code above. Pattern:

```rust
#[cfg(feature = "otel")]
bencher_otel::ApiMeter::increment(bencher_otel::oci::OciCounter::BlobGet);
```

---

## Phase 10: Conformance Testing

### Step 10.1: Create Test Infrastructure

Create `plus/bencher_oci/tests/conformance.rs`:

```rust
//! OCI Distribution Spec Conformance Tests
//!
//! These tests run the official OCI conformance test suite against our registry.

use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for conformance tests
struct ConformanceConfig {
    registry_url: String,
    namespace: String,
    crossmount_namespace: String,
    test_push: bool,
    test_pull: bool,
    test_content_discovery: bool,
    test_content_management: bool,
}

impl Default for ConformanceConfig {
    fn default() -> Self {
        Self {
            registry_url: "http://localhost:61016".to_owned(),
            namespace: "conformance/test".to_owned(),
            crossmount_namespace: "conformance/crossmount".to_owned(),
            test_push: true,
            test_pull: true,
            test_content_discovery: true,
            test_content_management: true,
        }
    }
}

/// Run the OCI conformance test binary
pub fn run_conformance_tests(config: ConformanceConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new("conformance.test");

    // Set environment variables
    cmd.env("OCI_ROOT_URL", &config.registry_url)
        .env("OCI_NAMESPACE", &config.namespace)
        .env("OCI_CROSSMOUNT_NAMESPACE", &config.crossmount_namespace)
        .env("OCI_TEST_PUSH", if config.test_push { "1" } else { "0" })
        .env("OCI_TEST_PULL", if config.test_pull { "1" } else { "0" })
        .env("OCI_TEST_CONTENT_DISCOVERY", if config.test_content_discovery { "1" } else { "0" })
        .env("OCI_TEST_CONTENT_MANAGEMENT", if config.test_content_management { "1" } else { "0" })
        .env("OCI_DEBUG", "1")
        .env("OCI_HIDE_SKIPPED_WORKFLOWS", "0")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status()?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Conformance tests failed with status: {}", status).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test that starts the server and runs conformance tests
    #[tokio::test]
    #[ignore] // Run with: cargo test --test conformance -- --ignored
    async fn test_oci_conformance() {
        // Start the API server in background
        let server_handle = tokio::spawn(async {
            // Your server startup code here
            // This would typically be extracted from main.rs
        });

        // Wait for server to be ready
        sleep(Duration::from_secs(2)).await;

        // Run conformance tests
        let config = ConformanceConfig::default();
        run_conformance_tests(config).expect("Conformance tests should pass");

        // Cleanup
        server_handle.abort();
    }
}
```

### Step 10.2: Create a Cargo Task for Conformance Tests

Add to `xtask/src/main.rs`:

```rust
fn run_oci_conformance() -> Result<()> {
    // Build the conformance test binary if not present
    if !Path::new("./conformance.test").exists() {
        println!("Building OCI conformance tests...");
        let status = Command::new("go")
            .args(["test", "-c", "-o", "conformance.test"])
            .current_dir("vendor/distribution-spec/conformance")
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to build conformance tests"));
        }
    }

    // Start the API server
    println!("Starting API server...");
    let mut server = Command::new("cargo")
        .args(["run", "--package", "bencher_api"])
        .spawn()?;

    // Wait for server to be ready
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Run conformance tests
    println!("Running OCI conformance tests...");
    let status = Command::new("./conformance.test")
        .env("OCI_ROOT_URL", "http://localhost:61016")
        .env("OCI_NAMESPACE", "conformance/test")
        .env("OCI_CROSSMOUNT_NAMESPACE", "conformance/crossmount")
        .env("OCI_TEST_PUSH", "1")
        .env("OCI_TEST_PULL", "1")
        .env("OCI_TEST_CONTENT_DISCOVERY", "1")
        .env("OCI_TEST_CONTENT_MANAGEMENT", "1")
        .env("OCI_DEBUG", "1")
        .status()?;

    // Cleanup
    server.kill()?;

    if status.success() {
        println!("All conformance tests passed!");
        Ok(())
    } else {
        Err(anyhow::anyhow!("Conformance tests failed"))
    }
}
```

### Step 10.3: Create Docker-based Test Runner

Create `docker/oci-conformance/docker-compose.yml`:

```yaml
version: '3.8'

services:
  registry:
    build:
      context: ../..
      dockerfile: docker/Dockerfile
    ports:
      - "61016:61016"
    environment:
      - BENCHER_API_PORT=61016
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:61016/v2/"]
      interval: 5s
      timeout: 5s
      retries: 10

  conformance:
    image: ghcr.io/opencontainers/distribution-spec/conformance:latest
    depends_on:
      registry:
        condition: service_healthy
    environment:
      - OCI_ROOT_URL=http://registry:61016
      - OCI_NAMESPACE=conformance/test
      - OCI_CROSSMOUNT_NAMESPACE=conformance/crossmount
      - OCI_TEST_PUSH=1
      - OCI_TEST_PULL=1
      - OCI_TEST_CONTENT_DISCOVERY=1
      - OCI_TEST_CONTENT_MANAGEMENT=1
      - OCI_DEBUG=1
    volumes:
      - ./reports:/reports
```

Create `docker/oci-conformance/run.sh`:

```bash
#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "Running OCI conformance tests..."
docker compose up --build --abort-on-container-exit --exit-code-from conformance

echo "Test reports available in ./reports/"
```

---

## Phase 11: CI Integration

### Step 11.1: Create GitHub Actions Workflow

Create `.github/workflows/oci-conformance.yml`:

```yaml
name: OCI Conformance

on:
  push:
    branches: [main]
    paths:
      - 'plus/bencher_oci/**'
      - 'services/api/**'
  pull_request:
    paths:
      - 'plus/bencher_oci/**'
      - 'services/api/**'

env:
  CARGO_TERM_COLOR: always

jobs:
  conformance:
    name: OCI Distribution Conformance
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Build API server
        run: cargo build --release --package bencher_api

      - name: Start API server
        run: |
          ./target/release/bencher_api &
          sleep 5
          curl -f http://localhost:61016/v2/ || exit 1

      - name: Run OCI Conformance Tests
        uses: opencontainers/distribution-spec@main
        env:
          OCI_ROOT_URL: http://localhost:61016
          OCI_NAMESPACE: conformance/test
          OCI_CROSSMOUNT_NAMESPACE: conformance/crossmount
          OCI_TEST_PUSH: 1
          OCI_TEST_PULL: 1
          OCI_TEST_CONTENT_DISCOVERY: 1
          OCI_TEST_CONTENT_MANAGEMENT: 1
          OCI_DEBUG: 1

      - name: Upload Conformance Report
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: oci-conformance-report
          path: |
            junit.xml
            report.html

      - name: Stop API server
        if: always()
        run: pkill bencher_api || true
```

### Step 11.2: Add to Existing CI

Update `.github/workflows/ci.yml` to include OCI conformance as part of the test suite:

```yaml
jobs:
  # ... existing jobs ...

  oci-conformance:
    name: OCI Conformance
    runs-on: ubuntu-latest
    needs: [build]  # Run after build succeeds

    steps:
      # ... same as above workflow ...
```

---

## Reference

### OCI Distribution Spec Endpoints Summary

| Endpoint                        | Method | Purpose               | Required       |
| ------------------------------- | ------ | --------------------- | -------------- |
| `/v2/`                          | GET    | Version check         | Yes            |
| `/v2/<name>/blobs/<digest>`     | HEAD   | Check blob exists     | Yes (Pull)     |
| `/v2/<name>/blobs/<digest>`     | GET    | Download blob         | Yes (Pull)     |
| `/v2/<name>/blobs/<digest>`     | DELETE | Delete blob           | No (Mgmt)      |
| `/v2/<name>/blobs/uploads/`     | POST   | Start upload          | Yes (Push)     |
| `/v2/<name>/blobs/uploads/<id>` | PATCH  | Upload chunk          | Yes (Push)     |
| `/v2/<name>/blobs/uploads/<id>` | PUT    | Complete upload       | Yes (Push)     |
| `/v2/<name>/blobs/uploads/<id>` | GET    | Get upload status     | Yes (Push)     |
| `/v2/<name>/blobs/uploads/<id>` | DELETE | Cancel upload         | No (Push)      |
| `/v2/<name>/manifests/<ref>`    | HEAD   | Check manifest exists | Yes (Pull)     |
| `/v2/<name>/manifests/<ref>`    | GET    | Download manifest     | Yes (Pull)     |
| `/v2/<name>/manifests/<ref>`    | PUT    | Upload manifest       | Yes (Push)     |
| `/v2/<name>/manifests/<ref>`    | DELETE | Delete manifest       | No (Mgmt)      |
| `/v2/<name>/tags/list`          | GET    | List tags             | No (Discovery) |
| `/v2/<name>/referrers/<digest>` | GET    | List referrers        | No (Discovery) |

### Error Codes

| Code                  | Description               |
| --------------------- | ------------------------- |
| BLOB_UNKNOWN          | Blob not found            |
| BLOB_UPLOAD_INVALID   | Upload error              |
| BLOB_UPLOAD_UNKNOWN   | Upload session not found  |
| DIGEST_INVALID        | Invalid digest format     |
| MANIFEST_BLOB_UNKNOWN | Referenced blob not found |
| MANIFEST_INVALID      | Invalid manifest format   |
| MANIFEST_UNKNOWN      | Manifest not found        |
| NAME_INVALID          | Invalid repository name   |
| NAME_UNKNOWN          | Repository not found      |
| SIZE_INVALID          | Invalid content size      |
| UNAUTHORIZED          | Authentication required   |
| DENIED                | Access denied             |
| UNSUPPORTED           | Unsupported operation     |

### Conformance Test Environment Variables

| Variable                    | Description                   |
| --------------------------- | ----------------------------- |
| OCI_ROOT_URL                | Registry base URL             |
| OCI_NAMESPACE               | Primary test repository       |
| OCI_CROSSMOUNT_NAMESPACE    | Cross-mount test repository   |
| OCI_USERNAME                | Auth username (optional)      |
| OCI_PASSWORD                | Auth password (optional)      |
| OCI_TEST_PUSH               | Enable push tests (1/0)       |
| OCI_TEST_PULL               | Enable pull tests (1/0)       |
| OCI_TEST_CONTENT_DISCOVERY  | Enable discovery tests (1/0)  |
| OCI_TEST_CONTENT_MANAGEMENT | Enable management tests (1/0) |
| OCI_DEBUG                   | Enable debug output (1/0)     |
| OCI_REPORT_DIR              | Report output directory       |

### Resources

- [OCI Distribution Spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md)
- [OCI Conformance Tests](https://github.com/opencontainers/distribution-spec/tree/main/conformance)
- [Dropshot Documentation](https://docs.rs/dropshot)
- [OpenTelemetry Rust](https://docs.rs/opentelemetry)

---

## Checklist

Use this checklist to track implementation progress:

- [ ] **Phase 1: Project Setup (Plus Feature)**
  - [ ] Create `plus/bencher_oci` crate in the Plus directory
  - [ ] Add to workspace members
  - [ ] Add as optional dependency to `services/api` with `plus` feature gate
  - [ ] Register with API server behind `#[cfg(feature = "plus")]`
  - [ ] Add `OciStorage` to `ApiContext` (Plus-gated)
  - [ ] Initialize OCI storage in server startup

- [ ] **Phase 2: Core Types and S3 Storage**
  - [ ] Implement `RepositoryName`, `Digest`, `Tag`, `Reference`
  - [ ] Implement `OciError` types
  - [ ] Add `JsonOci` config to `lib/bencher_json/src/system/config/plus/oci.rs`
  - [ ] Add `oci` field to `JsonPlus` struct
  - [ ] Implement `S3Arn` parser (reuse pattern from database backup)
  - [ ] Implement `OciStorage` using `aws-sdk-s3`
  - [ ] Implement `BlobStore` using S3
  - [ ] Implement `UploadStore` with session tracking
  - [ ] Implement `ManifestStore` with tag management

- [ ] **Phase 3: Base Endpoint**
  - [ ] `GET /v2/` returns 200 OK

- [ ] **Phase 4: Blob Push Endpoints**
  - [ ] `POST /v2/<name>/blobs/uploads/` - Initiate upload
  - [ ] `PATCH <location>` - Upload chunk
  - [ ] `PUT <location>?digest=` - Complete upload
  - [ ] `GET <location>` - Get upload status
  - [ ] `DELETE <location>` - Cancel upload
  - [ ] Cross-repository mount support

- [ ] **Phase 5: Manifest Push Endpoint**
  - [ ] `PUT /v2/<name>/manifests/<reference>` - Upload manifest
  - [ ] Validate referenced blobs exist

- [ ] **Phase 6: Pull Endpoints**
  - [ ] `HEAD /v2/<name>/blobs/<digest>`
  - [ ] `GET /v2/<name>/blobs/<digest>`
  - [ ] `HEAD /v2/<name>/manifests/<reference>`
  - [ ] `GET /v2/<name>/manifests/<reference>`

- [ ] **Phase 7: Content Discovery**
  - [ ] `GET /v2/<name>/tags/list` with pagination
  - [ ] `GET /v2/<name>/referrers/<digest>`

- [ ] **Phase 8: Content Management**
  - [ ] `DELETE /v2/<name>/blobs/<digest>`
  - [ ] `DELETE /v2/<name>/manifests/<digest>`

- [ ] **Phase 9: OTEL Metrics**
  - [ ] Add `OciCounter` enum
  - [ ] Instrument all endpoints

- [ ] **Phase 10: Conformance Testing**
  - [ ] Set up test infrastructure
  - [ ] Create cargo xtask command
  - [ ] Create Docker-based runner

- [ ] **Phase 11: CI Integration**
  - [ ] Add GitHub Actions workflow
  - [ ] Upload test artifacts
