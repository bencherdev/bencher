//! A multipart store over the local filesystem, which `object_store` does not give one.
//!
//! Each part is its own file beside the object, under `parts/<upload ID>/`.
//! Completion writes the parts in order into one staged file and renames it into
//! place, so the object appears whole or not at all.

use std::sync::Arc;

use async_trait::async_trait;
use futures::TryStreamExt as _;
use object_store::multipart::{MultipartStore, PartId};
use object_store::path::{Path, PathPart};
use object_store::{MultipartId, ObjectStore, ObjectStoreExt as _, PutPayload, PutResult, Result};

use crate::e_tag::{md5_hex, unquoted};

/// The directory beside an object that holds its uploads' parts
const PARTS: &str = "parts";

/// The local filesystem's multipart uploads, kept in the store it wraps.
#[derive(Debug)]
pub(crate) struct FileMultipart {
    store: Arc<dyn ObjectStore>,
}

impl FileMultipart {
    pub(crate) fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl MultipartStore for FileMultipart {
    async fn create_multipart(&self, _path: &Path) -> Result<MultipartId> {
        Ok(uuid::Uuid::new_v4().to_string())
    }

    /// Stores the part, and gives its MD5 as its `ETag`, as S3 does.
    async fn put_part(
        &self,
        path: &Path,
        id: &MultipartId,
        part_idx: usize,
        data: PutPayload,
    ) -> Result<PartId> {
        let content_id = format!("\"{}\"", md5_hex(data.iter()));
        self.store
            .put(&parts_prefix(path, id).join(part_idx.to_string()), data)
            .await?;
        Ok(PartId { content_id })
    }

    /// Fails as [`object_store::Error::NotFound`] when a part is missing, as it is
    /// once an earlier completion or an abort removed the parts, and as
    /// [`object_store::Error::Precondition`] when a part's bytes are not the ones its
    /// `ETag` names, as they are not once another request stored the same part.
    async fn complete_multipart(
        &self,
        path: &Path,
        id: &MultipartId,
        parts: Vec<PartId>,
    ) -> Result<PutResult> {
        let prefix = parts_prefix(path, id);
        // Dropped before it completes, the staged file is removed.
        let mut upload = self.store.put_multipart(path).await?;
        for (part_idx, part) in parts.iter().enumerate() {
            let data = self
                .store
                .get(&prefix.clone().join(part_idx.to_string()))
                .await?
                .bytes()
                .await?;
            if md5_hex([&data]) != unquoted(&part.content_id) {
                return Err(object_store::Error::Precondition {
                    path: path.to_string(),
                    source: format!("Part {part_idx} does not hold the bytes of its ETag").into(),
                });
            }
            upload.put_part(data.into()).await?;
        }
        let completed = upload.complete().await?;
        self.abort_multipart(path, id).await?;
        Ok(completed)
    }

    /// Removes the upload's parts, which never touches a completed object.
    async fn abort_multipart(&self, path: &Path, id: &MultipartId) -> Result<()> {
        let parts: Vec<Path> = self
            .store
            .list(Some(&parts_prefix(path, id)))
            .map_ok(|meta| meta.location)
            .try_collect()
            .await?;
        for part in parts {
            match self.store.delete(&part).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {},
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// The parts of upload `id` of the object at `path`, in a directory beside it.
fn parts_prefix(path: &Path, id: &MultipartId) -> Path {
    let mut segments: Vec<PathPart<'_>> = path.parts().collect();
    segments.pop();
    Path::from_iter(segments).join(PARTS).join(id.as_str())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use object_store::local::LocalFileSystem;
    use object_store::multipart::MultipartStore as _;
    use object_store::path::Path;
    use object_store::{ObjectStore, ObjectStoreExt as _};

    use super::FileMultipart;

    /// A part replaced after its `ETag` was taken, as a racing request replaces
    /// one, fails the completion, and nothing is written.
    #[tokio::test]
    async fn a_part_that_is_not_the_bytes_of_its_etag_fails_the_completion() {
        let dir = tempfile::tempdir().unwrap();
        let store: Arc<dyn ObjectStore> =
            Arc::new(LocalFileSystem::new_with_prefix(dir.path()).unwrap());
        let multipart = FileMultipart::new(Arc::clone(&store));
        let key = Path::from("session/data");
        let upload_id = multipart.create_multipart(&key).await.unwrap();
        let first = multipart
            .put_part(&key, &upload_id, 0, Bytes::from_static(b"first").into())
            .await
            .unwrap();
        let second = multipart
            .put_part(&key, &upload_id, 1, Bytes::from_static(b"second").into())
            .await
            .unwrap();
        multipart
            .put_part(&key, &upload_id, 1, Bytes::from_static(b"SECOND").into())
            .await
            .unwrap();

        multipart
            .complete_multipart(&key, &upload_id, vec![first, second])
            .await
            .unwrap_err();

        assert!(store.head(&key).await.is_err(), "nothing is written");
    }
}
