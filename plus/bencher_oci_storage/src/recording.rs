//! The object store the registry calls, which records each call it forwards.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use object_store::multipart::{MultipartStore, PartId};
use object_store::{
    CopyOptions, GetOptions, GetResult, ListResult, MultipartId, MultipartUpload, ObjectMeta,
    ObjectStore, PutMultipartOptions, PutOptions, PutPayload, PutResult, RenameOptions, Result,
    UploadPart, path::Path,
};

use crate::metrics::{self, StorageOperation};

/// Forwards every trait method to the inner store, provided methods included,
/// so each backend keeps its own implementation of each.
#[derive(Debug)]
pub(crate) struct RecordingStore {
    inner: Arc<dyn ObjectStore>,
}

impl RecordingStore {
    pub(crate) fn new(inner: Arc<dyn ObjectStore>) -> Self {
        Self { inner }
    }
}

impl fmt::Display for RecordingStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[async_trait]
#[deny(clippy::missing_trait_methods)]
impl ObjectStore for RecordingStore {
    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> Result<PutResult> {
        metrics::time(
            StorageOperation::Put,
            self.inner.put_opts(location, payload, opts),
        )
        .await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> Result<Box<dyn MultipartUpload>> {
        let inner = metrics::time(
            StorageOperation::CreateMultipart,
            self.inner.put_multipart_opts(location, opts),
        )
        .await?;
        Ok(Box::new(RecordingUpload { inner }))
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
        let operation = if options.head {
            StorageOperation::Head
        } else {
            StorageOperation::Get
        };
        metrics::time(operation, self.inner.get_opts(location, options)).await
    }

    async fn get_ranges(&self, location: &Path, ranges: &[Range<u64>]) -> Result<Vec<Bytes>> {
        metrics::time(
            StorageOperation::Get,
            self.inner.get_ranges(location, ranges),
        )
        .await
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, Result<Path>>,
    ) -> BoxStream<'static, Result<Path>> {
        metrics::time_stream(
            StorageOperation::Delete,
            self.inner.delete_stream(locations),
        )
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>> {
        metrics::time_stream(StorageOperation::List, self.inner.list(prefix))
    }

    fn list_with_offset(
        &self,
        prefix: Option<&Path>,
        offset: &Path,
    ) -> BoxStream<'static, Result<ObjectMeta>> {
        metrics::time_stream(
            StorageOperation::List,
            self.inner.list_with_offset(prefix, offset),
        )
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        metrics::time(
            StorageOperation::List,
            self.inner.list_with_delimiter(prefix),
        )
        .await
    }

    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        metrics::time(
            StorageOperation::Copy,
            self.inner.copy_opts(from, to, options),
        )
        .await
    }

    async fn rename_opts(&self, from: &Path, to: &Path, options: RenameOptions) -> Result<()> {
        metrics::time(
            StorageOperation::Rename,
            self.inner.rename_opts(from, to, options),
        )
        .await
    }
}

/// The multipart store the registry stages uploads in, which records each call it forwards.
pub(crate) struct RecordingMultipart {
    inner: Arc<dyn MultipartStore>,
}

impl RecordingMultipart {
    pub(crate) fn new(inner: Arc<dyn MultipartStore>) -> Self {
        Self { inner }
    }
}

#[async_trait]
#[deny(clippy::missing_trait_methods)]
impl MultipartStore for RecordingMultipart {
    async fn create_multipart(&self, path: &Path) -> Result<MultipartId> {
        metrics::time(
            StorageOperation::CreateMultipart,
            self.inner.create_multipart(path),
        )
        .await
    }

    async fn create_multipart_opts(
        &self,
        path: &Path,
        opts: PutMultipartOptions,
    ) -> Result<MultipartId> {
        metrics::time(
            StorageOperation::CreateMultipart,
            self.inner.create_multipart_opts(path, opts),
        )
        .await
    }

    async fn put_part(
        &self,
        path: &Path,
        id: &MultipartId,
        part_idx: usize,
        data: PutPayload,
    ) -> Result<PartId> {
        metrics::time(
            StorageOperation::PutPart,
            self.inner.put_part(path, id, part_idx, data),
        )
        .await
    }

    async fn complete_multipart(
        &self,
        path: &Path,
        id: &MultipartId,
        parts: Vec<PartId>,
    ) -> Result<PutResult> {
        metrics::time(
            StorageOperation::Complete,
            self.inner.complete_multipart(path, id, parts),
        )
        .await
    }

    async fn abort_multipart(&self, path: &Path, id: &MultipartId) -> Result<()> {
        metrics::time(
            StorageOperation::Abort,
            self.inner.abort_multipart(path, id),
        )
        .await
    }
}

/// A multipart upload that records each part, its completion, and an abort.
#[derive(Debug)]
struct RecordingUpload {
    inner: Box<dyn MultipartUpload>,
}

#[async_trait]
#[deny(clippy::missing_trait_methods)]
impl MultipartUpload for RecordingUpload {
    fn put_part(&mut self, data: PutPayload) -> UploadPart {
        Box::pin(metrics::time(
            StorageOperation::PutPart,
            self.inner.put_part(data),
        ))
    }

    async fn complete(&mut self) -> Result<PutResult> {
        metrics::time(StorageOperation::Complete, self.inner.complete()).await
    }

    async fn abort(&mut self) -> Result<()> {
        metrics::time(StorageOperation::Abort, self.inner.abort()).await
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::ops::Range;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::stream::{self, BoxStream, StreamExt as _, TryStreamExt as _};
    use object_store::memory::InMemory;
    use object_store::{
        CopyOptions, GetOptions, GetResult, ListResult, MultipartUpload, ObjectMeta, ObjectStore,
        ObjectStoreExt as _, PutMode, PutMultipartOptions, PutOptions, PutPayload, PutResult,
        RenameOptions, Result, UploadPart, path::Path,
    };

    use super::RecordingStore;

    /// A wrapper that left a provided method to its default body would call other
    /// trait methods in its place, replacing the backend's own implementation,
    /// and one that dropped a call's options would change what the call does.
    #[tokio::test]
    async fn every_trait_method_reaches_the_inner_store_once() {
        let log = CallLog::default();
        let store = RecordingStore::new(Arc::new(CountingStore {
            inner: Arc::new(InMemory::new()),
            log: log.clone(),
            broken_streams: false,
        }));
        let a = Path::from("a");
        let b = Path::from("b");

        store.put(&a, "hello".into()).await.unwrap();
        let exists = store
            .put_opts(&a, "again".into(), PutOptions::from(PutMode::Create))
            .await
            .unwrap_err();
        assert!(
            matches!(exists, object_store::Error::AlreadyExists { .. }),
            "a create-only put keeps its mode: {exists}"
        );
        store.get(&a).await.unwrap();
        store.head(&a).await.unwrap();
        store.get_ranges(&a, &[0..1, 2..3]).await.unwrap();
        store.copy(&a, &b).await.unwrap();
        store.rename(&b, &Path::from("c")).await.unwrap();
        store.list(None).try_collect::<Vec<_>>().await.unwrap();
        store
            .list_with_offset(None, &a)
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        store.list_with_delimiter(None).await.unwrap();
        store.delete(&a).await.unwrap();
        let mut upload = store.put_multipart(&b).await.unwrap();
        upload.put_part("part".into()).await.unwrap();
        upload.complete().await.unwrap();
        let mut upload = store.put_multipart(&a).await.unwrap();
        upload.abort().await.unwrap();

        assert_eq!(
            log.calls(),
            [
                "put_opts",
                "put_opts",
                "get_opts",
                "get_opts head",
                "get_ranges",
                "copy_opts",
                "rename_opts",
                "list",
                "list_with_offset",
                "list_with_delimiter",
                "delete_stream",
                "put_multipart_opts",
                "put_part",
                "complete",
                "put_multipart_opts",
                "abort",
            ],
            "each call reaches its own method on the inner store"
        );
    }

    /// A stream that fails part way must fail for its caller, not end short as if
    /// it were whole.
    #[tokio::test]
    async fn a_stream_error_reaches_the_caller() {
        let inner = InMemory::new();
        let a = Path::from("a");
        let b = Path::from("b");
        inner.put(&a, "a".into()).await.unwrap();
        inner.put(&b, "b".into()).await.unwrap();
        let store = RecordingStore::new(Arc::new(CountingStore {
            inner: Arc::new(inner),
            log: CallLog::default(),
            broken_streams: true,
        }));

        store.list(None).try_collect::<Vec<_>>().await.unwrap_err();
        store
            .list_with_offset(None, &a)
            .try_collect::<Vec<_>>()
            .await
            .unwrap_err();
        store
            .delete_stream(stream::iter([Ok(a), Ok(b)]).boxed())
            .try_collect::<Vec<_>>()
            .await
            .unwrap_err();
    }

    /// An object store that logs each trait method called on it, then passes the call on.
    ///
    /// With `broken_streams`, each stream it answers fails after its first item.
    #[derive(Debug)]
    struct CountingStore {
        inner: Arc<dyn ObjectStore>,
        log: CallLog,
        broken_streams: bool,
    }

    impl CountingStore {
        fn pass_on<T>(&self, stream: BoxStream<'static, Result<T>>) -> BoxStream<'static, Result<T>>
        where
            T: Send + 'static,
        {
            if self.broken_streams {
                stream
                    .take(1)
                    .chain(stream::once(async {
                        Err(object_store::Error::Generic {
                            store: "counting",
                            source: "the stream broke".into(),
                        })
                    }))
                    .boxed()
            } else {
                stream
            }
        }
    }

    impl fmt::Display for CountingStore {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Counting({})", self.inner)
        }
    }

    #[async_trait]
    impl ObjectStore for CountingStore {
        async fn put_opts(
            &self,
            location: &Path,
            payload: PutPayload,
            opts: PutOptions,
        ) -> Result<PutResult> {
            self.log.push("put_opts");
            self.inner.put_opts(location, payload, opts).await
        }

        async fn put_multipart_opts(
            &self,
            location: &Path,
            opts: PutMultipartOptions,
        ) -> Result<Box<dyn MultipartUpload>> {
            self.log.push("put_multipart_opts");
            let inner = self.inner.put_multipart_opts(location, opts).await?;
            Ok(Box::new(CountingUpload {
                inner,
                log: self.log.clone(),
            }))
        }

        async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
            self.log.push(if options.head {
                "get_opts head"
            } else {
                "get_opts"
            });
            self.inner.get_opts(location, options).await
        }

        async fn get_ranges(&self, location: &Path, ranges: &[Range<u64>]) -> Result<Vec<Bytes>> {
            self.log.push("get_ranges");
            self.inner.get_ranges(location, ranges).await
        }

        fn delete_stream(
            &self,
            locations: BoxStream<'static, Result<Path>>,
        ) -> BoxStream<'static, Result<Path>> {
            self.log.push("delete_stream");
            self.pass_on(self.inner.delete_stream(locations))
        }

        fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>> {
            self.log.push("list");
            self.pass_on(self.inner.list(prefix))
        }

        fn list_with_offset(
            &self,
            prefix: Option<&Path>,
            offset: &Path,
        ) -> BoxStream<'static, Result<ObjectMeta>> {
            self.log.push("list_with_offset");
            self.pass_on(self.inner.list_with_offset(prefix, offset))
        }

        async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
            self.log.push("list_with_delimiter");
            self.inner.list_with_delimiter(prefix).await
        }

        async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
            self.log.push("copy_opts");
            self.inner.copy_opts(from, to, options).await
        }

        async fn rename_opts(&self, from: &Path, to: &Path, options: RenameOptions) -> Result<()> {
            self.log.push("rename_opts");
            self.inner.rename_opts(from, to, options).await
        }
    }

    #[derive(Debug)]
    struct CountingUpload {
        inner: Box<dyn MultipartUpload>,
        log: CallLog,
    }

    #[async_trait]
    impl MultipartUpload for CountingUpload {
        fn put_part(&mut self, data: PutPayload) -> UploadPart {
            self.log.push("put_part");
            self.inner.put_part(data)
        }

        async fn complete(&mut self) -> Result<PutResult> {
            self.log.push("complete");
            self.inner.complete().await
        }

        async fn abort(&mut self) -> Result<()> {
            self.log.push("abort");
            self.inner.abort().await
        }
    }

    /// The calls a [`CountingStore`] received, in order.
    #[derive(Debug, Default, Clone)]
    struct CallLog(Arc<Mutex<Vec<&'static str>>>);

    impl CallLog {
        fn push(&self, call: &'static str) {
            self.0.lock().unwrap().push(call);
        }

        fn calls(&self) -> Vec<&'static str> {
            self.0.lock().unwrap().clone()
        }
    }
}
