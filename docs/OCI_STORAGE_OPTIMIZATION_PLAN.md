# OCI Storage Optimization Plan

This document outlines a plan to address three performance concerns in the OCI storage implementation.

## Overview of Concerns

1. **Memory-intensive digest verification** - Both S3 and local backends load entire blobs into memory for digest verification at upload completion
2. **O(n²) S3 buffer appends** - S3 backend reads/writes the entire buffer on each chunk append
3. **N+1 referrer queries** - Listing referrers makes one request per referrer object

---

## Concern 1: Memory-Intensive Digest Verification

### Current Behavior

**S3** (`storage.rs:814-829`):
```rust
// Download and verify digest
let response = self.client.get_object()...
let data = response.body.collect().await...
let mut hasher = Sha256::new();
hasher.update(&data);
```

**Local** (`local.rs:227-243`):
```rust
let data = fs::read(&data_path).await...
let mut hasher = Sha256::new();
hasher.update(&data);
```

Both implementations load the entire blob into memory to compute the SHA256 digest.
For multi-GB blobs, this causes memory pressure and potential OOM.

### Solution: Incremental Digest Computation

**Approach**: Compute the digest incrementally during `append_upload`, storing it in the upload state.
At completion, simply verify the accumulated digest matches the expected one—no re-download needed.

#### Changes Required

**1. Add running hash to `UploadState`**

For S3 (`storage.rs`):
```rust
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    s3_upload_id: String,
    repository: String,
    parts: Vec<CompletedPartInfo>,
    // NEW: Serialized hasher state (SHA256 has fixed-size state)
    digest_state: Vec<u8>,
}
```

For local (`local.rs`):
```rust
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    repository: String,
    size: u64,
    // NEW: Serialized hasher state
    digest_state: Vec<u8>,
}
```

**2. Update `append_upload` to feed data through hasher**

```rust
pub async fn append_upload(&self, upload_id: &UploadId, data: Bytes) -> Result<u64, OciStorageError> {
    let mut state = self.load_upload_state(upload_id).await?;

    // Restore hasher from state
    let mut hasher = restore_hasher(&state.digest_state);

    // Update hash with new data
    hasher.update(&data);

    // ... existing append logic ...

    // Save hasher state
    state.digest_state = serialize_hasher(&hasher);
    self.save_upload_state(upload_id, &state).await?;

    Ok(total_size)
}
```

**3. Update `complete_upload` to verify without re-reading**

```rust
pub async fn complete_upload(
    &self,
    upload_id: &UploadId,
    expected_digest: &Digest,
) -> Result<Digest, OciStorageError> {
    let state = self.load_upload_state(upload_id).await?;

    // Finalize the accumulated hash
    let hasher = restore_hasher(&state.digest_state);
    let hash = hasher.finalize();
    let actual_digest = Digest::sha256(&hex::encode(hash));

    // Verify digest matches - NO DOWNLOAD NEEDED
    if actual_digest.as_str() != expected_digest.as_str() {
        self.cleanup_upload(upload_id).await;
        return Err(OciStorageError::DigestMismatch { ... });
    }

    // Complete the upload (S3: complete multipart, Local: move file)
    // ...
}
```

**4. Hasher serialization helpers**

The `sha2` crate's `Sha256` hasher doesn't directly support serialization, but we can use a wrapper:

```rust
use sha2::{Sha256, Digest as _};

fn serialize_hasher(hasher: &Sha256) -> Vec<u8> {
    // sha2 0.10+ supports Clone, so we can clone and finalize a copy
    // For state persistence, we need to track the data ourselves or use a
    // streaming approach with a custom accumulator
    //
    // Alternative: Use `ring` or `openssl` which support state export
    todo!("Implement based on chosen crypto library")
}
```

**Alternative approach**: If hasher serialization is complex, store an incremental checksum file:
- Append each chunk to a separate temp file AND update a running hash in memory
- Write the hash state to disk periodically (or after each chunk for safety)
- This is what the local backend essentially does with file appends

### Complexity: Medium
### Priority: High (blocks large blob uploads)

---

## Concern 2: O(n²) S3 Buffer Appends

### Current Behavior

S3 multipart requires 5MB minimum part size. For smaller chunks, the current implementation:
1. Downloads the existing buffer from S3
2. Appends new data in memory
3. Uploads the combined buffer back to S3

For N chunks of size K each, this is O(N² × K) in network I/O.

### Solution: Chunked Buffer Storage

**Approach**: Store each incoming chunk as a separate numbered S3 object. At completion, combine them.

#### Changes Required

**1. New key structure for buffer chunks**

```rust
/// Returns the S3 key for a buffer chunk
fn upload_buffer_chunk_key(&self, upload_id: &UploadId, chunk_num: u32) -> String {
    format!("{}/{}/buffer/{:08}", self.global_prefix(), upload_id, chunk_num)
}
```

**2. Update `UploadState` to track buffer chunks**

```rust
#[derive(Debug, Serialize, Deserialize)]
struct UploadState {
    s3_upload_id: String,
    repository: String,
    parts: Vec<CompletedPartInfo>,
    // NEW: Number of buffer chunks stored
    buffer_chunk_count: u32,
    // NEW: Total bytes in buffer chunks
    buffer_size: u64,
}
```

**3. Simplify `append_upload`**

```rust
pub async fn append_upload(&self, upload_id: &UploadId, data: Bytes) -> Result<u64, OciStorageError> {
    let mut state = self.load_upload_state(upload_id).await?;

    // Store this chunk directly (O(1) operation)
    let chunk_key = self.upload_buffer_chunk_key(upload_id, state.buffer_chunk_count);
    self.client.put_object()
        .bucket(&self.config.bucket_arn)
        .key(&chunk_key)
        .body(data.clone().into())
        .send()
        .await?;

    state.buffer_chunk_count += 1;
    state.buffer_size += data.len() as u64;

    // If we've accumulated enough, flush to multipart parts
    if state.buffer_size >= MIN_PART_SIZE {
        self.flush_buffer_to_part(upload_id, &mut state).await?;
    }

    self.save_upload_state(upload_id, &state).await?;
    Ok(state.completed_size() + state.buffer_size)
}
```

**4. New buffer flush function**

```rust
async fn flush_buffer_to_part(&self, upload_id: &UploadId, state: &mut UploadState) -> Result<(), OciStorageError> {
    // Read all buffer chunks
    let mut combined = Vec::with_capacity(state.buffer_size as usize);
    for i in 0..state.buffer_chunk_count {
        let chunk_key = self.upload_buffer_chunk_key(upload_id, i);
        let resp = self.client.get_object()
            .bucket(&self.config.bucket_arn)
            .key(&chunk_key)
            .send()
            .await?;
        let data = resp.body.collect().await?;
        combined.extend_from_slice(&data.into_bytes());
    }

    // Upload as multipart parts
    while combined.len() >= MIN_PART_SIZE {
        let part_data: Vec<u8> = combined.drain(..MIN_PART_SIZE).collect();
        self.upload_part(upload_id, state, part_data).await?;
    }

    // Store remainder as new buffer chunk (if any)
    // ... cleanup old chunks, store remainder ...

    Ok(())
}
```

### Complexity: Medium
### Priority: Medium (affects chunked uploads with small pieces)

---

## Concern 3: N+1 Referrer Queries

### Current Behavior

Listing referrers (`storage.rs:1282-1355`):
1. `list_objects_v2` to get all referrer keys
2. For each key, `get_object` to fetch the descriptor JSON

For N referrers, this makes N+1 S3 requests.

### Solution Options

#### Option A: Parallel Fetches (Quick Win)

Use `futures::stream` to fetch all descriptors concurrently with bounded parallelism.

```rust
use futures::stream::{self, StreamExt};

pub async fn list_referrers(...) -> Result<Vec<serde_json::Value>, OciStorageError> {
    // ... list objects ...

    let keys: Vec<String> = contents.iter()
        .filter_map(|obj| obj.key.clone())
        .collect();

    // Fetch all descriptors in parallel (max 10 concurrent)
    let descriptors: Vec<_> = stream::iter(keys)
        .map(|key| self.fetch_descriptor(key))
        .buffer_unordered(10)
        .filter_map(|result| async { result.ok() })
        .collect()
        .await;

    // Apply filter
    Ok(descriptors.into_iter()
        .filter(|d| matches_filter(d, artifact_type_filter))
        .collect())
}
```

**Complexity**: Low
**Improvement**: N+1 sequential → 1 + ceil(N/10) parallel batches

#### Option B: Referrer Index File (Better for large N)

Maintain a single JSON file listing all referrers for a subject.

**On manifest PUT with subject**:
```rust
// Read current index (or empty)
let index_key = format!("{}/referrers-index/{}", repo, subject_digest);
let mut index: Vec<Descriptor> = self.read_index(&index_key).await.unwrap_or_default();

// Append new referrer
index.push(new_descriptor);

// Write back
self.write_index(&index_key, &index).await?;
```

**On list_referrers**:
```rust
// Single read
let index = self.read_index(&index_key).await?;
Ok(index.into_iter()
    .filter(|d| matches_filter(d, artifact_type_filter))
    .collect())
```

**Drawbacks**:
- Need to handle concurrent updates (read-modify-write race)
- Need to handle manifest deletes (remove from index)
- Index can grow large for manifests with many referrers

**Complexity**: Medium-High
**Improvement**: N+1 → 1 request

#### Recommendation

Start with **Option A** (parallel fetches). It's low-risk, improves performance significantly, and can be done quickly. Consider Option B only if referrer counts become very large (100+).

---

## Implementation Order

1. **Concern 3 (N+1 referrers)** - Quick win with parallel fetches, low risk
2. **Concern 1 (Digest verification)** - High priority, requires careful design
3. **Concern 2 (O(n²) buffer)** - Medium priority, only affects specific upload patterns

## Testing Considerations

- Add benchmarks for large blob uploads (1GB+)
- Add benchmarks for chunked uploads with many small pieces
- Add benchmarks for repositories with many referrers
- Ensure digest verification still works correctly after streaming changes
- Test upload resume scenarios with new state format

## Migration

The `UploadState` structure changes are backwards-incompatible. Options:
1. Version the state format, migrate on read
2. Fail gracefully for in-progress uploads during upgrade (they'll need to restart)
3. Add new fields as optional with defaults

Recommendation: Option 2 for simplicity—in-progress uploads during a deployment are rare and can be restarted.
