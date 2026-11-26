//! An upload session's bytes, streamed into a multipart upload, and its finish as a blob.
//!
//! A request body fills parts of the session's part size, and up to
//! [`PARTS_IN_FLIGHT`] of them upload at once while the body still arrives. The
//! bytes past the last full part are the tail, an object of their own. Each
//! request commits once, at its end, so a request that fails part way leaves the
//! session as the last commit left it, and the client resends from there.

use std::num::NonZeroU64;
use std::pin::pin;
use std::sync::Arc;

use bencher_json::ProjectUuid;
use bytes::Bytes;
use futures::{Stream, StreamExt as _};
use object_store::multipart::{MultipartStore as _, PartId};
use object_store::path::Path;
use object_store::{
    MultipartId, ObjectMeta, ObjectStore as _, ObjectStoreExt as _, PutMode, PutOptions, PutPayload,
};
use sha2::{Digest as _, Sha256};
use slog::Logger;
use tokio::task::{JoinError, JoinSet};

use super::session::{
    Finished, HashState, Phase, Progress, Promotion, SessionState, Tail, UploadSession, generation,
};
use super::{OciStorage, OciStorageError, is_not_found, report_cleanup_error, storage_error};
use crate::buffers::{Buffer, FILL_DEADLINE};
use crate::e_tag::{md5_hex, unquoted};
use crate::metrics::{self, FinalizeOutcome};
use crate::part_copy::{
    Completion, CopyError, PartCopy, is_invalid_part, is_no_such_upload, multipart_e_tag,
};
use crate::recording::RecordingMultipart;
use crate::types::{Digest, UploadId};

/// Most parts one request uploads at once
const PARTS_IN_FLIGHT: usize = 4;

/// Most times an object or a part is put when S3 gives it an `ETag` that is not its MD5
const E_TAG_ATTEMPTS: u32 = 2;

/// What a request body appended to an upload session.
#[derive(Debug, Clone, Copy)]
pub struct Appended {
    /// The bytes the body held
    pub received: u64,
    /// The bytes the session holds, with this body's when it committed
    pub offset: u64,
    /// Whether this body's bytes are in the session. A request that loses the
    /// commit to another adds nothing.
    pub committed: bool,
}

/// A finished upload.
#[derive(Debug, Clone)]
pub struct Completed {
    pub digest: Digest,
    /// The bytes the final request body held
    pub received: u64,
}

/// A failed upload request, with a failure of its request body kept apart.
#[derive(Debug)]
pub enum UploadWriteError<E> {
    /// The request body failed part way, as it does when the client goes away.
    Body(E),
    Storage(OciStorageError),
}

impl<E> From<OciStorageError> for UploadWriteError<E> {
    fn from(error: OciStorageError) -> Self {
        Self::Storage(error)
    }
}

/// The requests of upload sessions, over the storage that holds them.
#[derive(Clone, Copy)]
pub(super) struct Uploads<'a> {
    storage: &'a OciStorage,
}

impl<'a> Uploads<'a> {
    pub(super) fn new(storage: &'a OciStorage) -> Self {
        Self { storage }
    }

    /// Loads a session's newest state.
    ///
    /// A session with no state this binary can read is unknown: one in the old
    /// layout, one whose hash state has another tag, or one already removed.
    pub(super) async fn load(
        &self,
        upload_id: &UploadId,
    ) -> Result<UploadSession, OciStorageError> {
        let unknown = || OciStorageError::UploadNotFound(upload_id.to_string());
        let states = self
            .storage
            .list_prefix(&self.storage.upload_states_prefix(upload_id))
            .await?;
        let (generation, key) = states
            .into_iter()
            .filter_map(|meta| Some((generation(&meta.location)?, meta.location)))
            .max_by_key(|(generation, _key)| *generation)
            .ok_or_else(unknown)?;
        let data = self.storage.get_bytes(&key, unknown()).await?;
        let state: SessionState = serde_json::from_slice(&data).map_err(|_e| unknown())?;
        if let Phase::Open(progress) = &state.phase
            && progress.hash.hasher().is_none()
        {
            return Err(unknown());
        }
        Ok(UploadSession::new(upload_id.clone(), generation, state))
    }

    /// Creates a session's first state.
    pub(super) async fn create(
        &self,
        upload_id: &UploadId,
        state: SessionState,
    ) -> Result<(), OciStorageError> {
        let key = self.storage.upload_state_generation_key(upload_id, 0);
        if self.put_state(&key, &state).await? {
            Ok(())
        } else {
            Err(OciStorageError::Storage(format!(
                "Upload session {upload_id} already exists"
            )))
        }
    }

    /// Commits the session's next state, or returns `None` when another request
    /// committed one first.
    pub(super) async fn commit(
        &self,
        session: &UploadSession,
        state: SessionState,
    ) -> Result<Option<UploadSession>, OciStorageError> {
        let generation = session.generation() + 1;
        let key = self
            .storage
            .upload_state_generation_key(session.id(), generation);
        Ok(self
            .put_state(&key, &state)
            .await?
            .then(|| UploadSession::new(session.id().clone(), generation, state)))
    }

    /// Puts a state at `key` only if no state is there, and returns whether the
    /// state there is this request's. `object_store` retries a put whose answer was
    /// a 5xx, or whose connection closed, and a retry of a put that the store kept
    /// finds `key` taken, so the bytes at `key` tell this request's state from another's.
    async fn put_state(&self, key: &Path, state: &SessionState) -> Result<bool, OciStorageError> {
        let json = Bytes::from(state_json(state)?);
        match self
            .storage
            .store
            .put_opts(key, json.clone().into(), PutOptions::from(PutMode::Create))
            .await
        {
            Ok(_result) => Ok(true),
            // A failed read leaves the winner unknown, so the request fails as one
            // whose answer was lost does, and keeps the upload it opened.
            Err(
                object_store::Error::AlreadyExists { .. }
                | object_store::Error::Precondition { .. },
            ) => Ok(self.storage.read(key).await? == json),
            Err(e) => Err(storage_error(&e)),
        }
    }

    pub(super) async fn session(
        &self,
        upload_id: &UploadId,
        repository: &ProjectUuid,
    ) -> Result<UploadSession, OciStorageError> {
        let session = self.load(upload_id).await?;
        if session.state().repository == *repository {
            Ok(session)
        } else {
            Err(OciStorageError::UploadNotFound(upload_id.to_string()))
        }
    }

    pub(super) async fn append<S, E>(
        &self,
        session: UploadSession,
        body: S,
    ) -> Result<Appended, UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let Phase::Open(progress) = &session.state().phase else {
            return Err(OciStorageError::BlobUploadInvalidContent(
                "The upload is already complete".to_owned(),
            )
            .into());
        };
        let mut writer = PartWriter::new(*self, &session, progress);
        let appended = self
            .append_parts(&session, progress, &mut writer, body)
            .await;
        self.abort_uncommitted(&writer).await;
        appended
    }

    async fn append_parts<S, E>(
        &self,
        session: &UploadSession,
        progress: &Progress,
        writer: &mut PartWriter<'_>,
        body: S,
    ) -> Result<Appended, UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let written = writer.write_body(body).await;
        let parts = writer.settle().await;
        written?;
        let parts = parts?;
        if writer.received == 0 {
            return Ok(Appended {
                received: 0,
                offset: progress.offset,
                committed: true,
            });
        }

        let tail = writer.store_tail().await?;
        let state = SessionState {
            upload: writer.upload.clone(),
            phase: Phase::Open(Progress {
                offset: writer.offset,
                parts,
                hash: HashState::of(&writer.hasher),
                tail,
            }),
            ..session.state().clone()
        };
        let committed = writer.commit(session, state).await?.is_some();
        let offset = if committed {
            writer.offset
        } else {
            self.load(session.id()).await?.offset()
        };
        Ok(Appended {
            received: writer.received,
            offset,
            committed,
        })
    }

    pub(super) async fn complete<S, E>(
        &self,
        session: UploadSession,
        digest: &Digest,
        body: S,
    ) -> Result<Completed, UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let resumed = !matches!(session.state().phase, Phase::Open(_));
        let finished = self.finalize(session, digest, body).await;
        match &finished {
            Ok((_completed, size)) => {
                metrics::blob_upload(*size);
                metrics::finalize(if resumed {
                    FinalizeOutcome::Resumed
                } else {
                    FinalizeOutcome::Ok
                });
            },
            Err(UploadWriteError::Storage(error)) => metrics::finalize(finalize_outcome(error)),
            Err(UploadWriteError::Body(_)) => {},
        }
        finished.map(|(completed, _size)| completed)
    }

    pub(super) async fn cancel(&self, upload_id: &UploadId) -> Result<(), OciStorageError> {
        let session = self.load(upload_id).await?;
        if let Phase::Open(_) = session.state().phase {
            self.storage
                .sweeper()
                .remove(
                    &self.storage.upload_prefix(upload_id),
                    Some(session.state()),
                )
                .await;
        }
        Ok(())
    }

    async fn finalize<S, E>(
        &self,
        session: UploadSession,
        digest: &Digest,
        body: S,
    ) -> Result<(Completed, u64), UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let (mut step, received) = if let Phase::Open(progress) = &session.state().phase {
            let progress = progress.clone();
            self.finish_open(session, &progress, digest, body).await?
        } else {
            (Step::Next(Box::new(session)), 0)
        };
        let mut reopened = false;
        loop {
            match step {
                Step::Next(session) => step = self.advance(*session, digest, &mut reopened).await?,
                Step::Done(size) => {
                    return Ok((
                        Completed {
                            digest: digest.clone(),
                            received,
                        },
                        size,
                    ));
                },
            }
        }
    }

    /// Streams the final body, checks the digest, and either puts a blob of less
    /// than one part whole at its digest key, or commits the `Completing` phase.
    async fn finish_open<S, E>(
        &self,
        session: UploadSession,
        progress: &Progress,
        digest: &Digest,
        body: S,
    ) -> Result<(Step, u64), UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let mut writer = PartWriter::new(*self, &session, progress);
        let finished = self.finish_parts(session, &mut writer, digest, body).await;
        self.abort_uncommitted(&writer).await;
        finished
    }

    async fn finish_parts<S, E>(
        &self,
        session: UploadSession,
        writer: &mut PartWriter<'_>,
        digest: &Digest,
        body: S,
    ) -> Result<(Step, u64), UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let mut written = writer.write_body(body).await;
        if written.is_ok() {
            written = writer.read_tail().await.map_err(UploadWriteError::from);
        }
        let parts = writer.settle().await;
        written?;
        let mut parts = parts?;
        let received = writer.received;
        let size = writer.offset;

        if size == 0 {
            self.cancel_open(session.id()).await;
            return Err(OciStorageError::BlobUploadInvalidContent(
                "Cannot complete upload with no data".to_owned(),
            )
            .into());
        }
        let actual = sha256_digest(std::mem::take(&mut writer.hasher))?;
        if actual != *digest {
            self.cancel_open(session.id()).await;
            return Err(OciStorageError::DigestMismatch {
                expected: digest.to_string(),
                actual: actual.to_string(),
            }
            .into());
        }

        let (chunks, buffer) = writer.buffer.take();
        let Some(upload) = writer.upload.clone() else {
            self.put_whole_blob(&session, digest, chunks).await?;
            drop(buffer);
            self.cleanup_session(session.id()).await;
            return Ok((Step::Done(size), received));
        };
        if !chunks.is_empty() {
            let part = put_part(
                &self.storage.multipart,
                &self.storage.log,
                &writer.key,
                &upload,
                parts.len(),
                chunks,
                self.storage.part_copy.is_some(),
            )
            .await?;
            parts.push(part.content_id);
        }
        drop(buffer);

        let state = SessionState {
            upload: Some(upload),
            phase: Phase::Completing(Finished {
                digest: digest.to_string(),
                size,
                parts,
            }),
            ..session.state().clone()
        };
        let next = match writer.commit(&session, state).await? {
            Some(next) => next,
            None => self.load(session.id()).await?,
        };
        Ok((Step::Next(Box::new(next)), received))
    }

    /// Takes a session past its digest check one phase further.
    async fn advance(
        &self,
        session: UploadSession,
        digest: &Digest,
        reopened: &mut bool,
    ) -> Result<Step, OciStorageError> {
        match session.state().phase.clone() {
            Phase::Open(_) => Err(OciStorageError::Storage(
                "Another request appended to the upload while it finished".to_owned(),
            )),
            Phase::Completing(finished) => {
                check_digest(&finished, digest)?;
                self.complete_staging(session, &finished, digest).await
            },
            Phase::Promoting(promotion) => {
                check_digest(&promotion.finished, digest)?;
                self.promote(session, &promotion, digest, reopened).await
            },
        }
    }

    /// Completes the staging upload, checks the staging object, and commits the
    /// `Promoting` phase.
    async fn complete_staging(
        &self,
        session: UploadSession,
        finished: &Finished,
        digest: &Digest,
    ) -> Result<Step, OciStorageError> {
        let Some(upload) = &session.state().upload else {
            return Err(OciStorageError::Storage(
                "A completing upload has no staging upload".to_owned(),
            ));
        };
        let staging = self.storage.upload_data_key(session.id());
        let parts = finished
            .parts
            .iter()
            .map(|e_tag| PartId {
                content_id: e_tag.clone(),
            })
            .collect();
        let completed = self
            .storage
            .multipart
            .complete_multipart(&staging, upload, parts)
            .await;
        // A completion answered before, or an abort, leaves the upload gone, and the
        // staging object tells which. A part that another request replaced leaves
        // an upload that no retry can complete, so the client starts the blob again.
        if let Err(e) = completed
            && !is_upload_gone(&e)
        {
            return Err(if is_part_refused(&e) {
                OciStorageError::UploadNotFound(session.id().to_string())
            } else {
                storage_error(&e)
            });
        }

        let Some(meta) = self.head_object(&staging).await? else {
            // Without its staging object, the session was either promoted by
            // another request and cleaned up, or aborted.
            return self.promoted_elsewhere(&session, finished, digest).await;
        };
        if meta.size != finished.size {
            return Err(OciStorageError::Storage(format!(
                "{staging} holds {} bytes, not the {} bytes uploaded",
                meta.size, finished.size
            )));
        }
        if self.storage.part_copy.is_some() {
            check_completed_e_tag(&staging, meta.e_tag.as_deref(), &finished.parts)?;
        }

        let blob_key = self.storage.blob_key(&session.state().repository, digest);
        let destination = match &self.storage.part_copy {
            Some(part_copy) => Some(part_copy.open(&blob_key).await?),
            None => None,
        };
        let state = SessionState {
            phase: Phase::Promoting(Promotion {
                finished: finished.clone(),
                staging_e_tag: meta.e_tag,
                destination: destination.clone(),
            }),
            ..session.state().clone()
        };
        if let Some(next) = self.commit(&session, state).await? {
            return Ok(Step::Next(Box::new(next)));
        }
        if let (Some(part_copy), Some(destination)) = (&self.storage.part_copy, &destination) {
            self.abort_destination(part_copy, &blob_key, destination)
                .await;
        }
        Ok(Step::Next(Box::new(self.load(session.id()).await?)))
    }

    /// Moves the staging object to the digest key: by parts on S3, by a rename elsewhere.
    async fn promote(
        &self,
        session: UploadSession,
        promotion: &Promotion,
        digest: &Digest,
        reopened: &mut bool,
    ) -> Result<Step, OciStorageError> {
        let size = promotion.finished.size;
        let blob_key = self.storage.blob_key(&session.state().repository, digest);
        if self.holds(&blob_key, size).await? {
            // Abort never touches a completed object, so a destination another
            // request completed stays.
            if let (Some(part_copy), Some(destination)) =
                (&self.storage.part_copy, &promotion.destination)
            {
                self.abort_destination(part_copy, &blob_key, destination)
                    .await;
            }
            self.cleanup_session(session.id()).await;
            return Ok(Step::Done(size));
        }
        let staging = self.storage.upload_data_key(session.id());
        let Some(part_copy) = &self.storage.part_copy else {
            return match self.storage.store.rename(&staging, &blob_key).await {
                Ok(()) => {
                    self.cleanup_session(session.id()).await;
                    Ok(Step::Done(size))
                },
                Err(e) if is_not_found(&e) => {
                    self.promoted_elsewhere(&session, &promotion.finished, digest)
                        .await
                },
                Err(e) => Err(storage_error(&e)),
            };
        };
        let Some(destination) = &promotion.destination else {
            return Err(OciStorageError::Storage(
                "A promoting upload has no destination upload".to_owned(),
            ));
        };

        let part_size = NonZeroU64::new(session.state().part_size).ok_or_else(|| {
            OciStorageError::Storage("An upload has a part size of zero".to_owned())
        })?;
        let parts = match part_copy
            .copy_parts(&staging, &blob_key, destination, size, part_size)
            .await
        {
            Ok(parts) => parts,
            Err(CopyError::UploadGone) => {
                return self
                    .reopen_destination(session, promotion, part_copy, &blob_key, reopened)
                    .await;
            },
            Err(CopyError::Storage(error)) => return Err(error),
        };
        if let Completion::Completed { e_tag } = part_copy
            .complete(&blob_key, destination, parts, size)
            .await?
            && promotion.staging_e_tag.as_deref().map(unquoted) != Some(unquoted(&e_tag))
        {
            if let Err(e) = self.storage.delete_key(&blob_key).await {
                report_cleanup_error(&self.storage.log, "promote: delete a mismatched blob", &e);
            }
            return Err(OciStorageError::EtagMismatch(format!(
                "Promoted {blob_key} with ETag {e_tag}, not its staging ETag {}",
                promotion.staging_e_tag.as_deref().unwrap_or_default()
            )));
        }
        self.cleanup_session(session.id()).await;
        Ok(Step::Done(size))
    }

    /// Replaces a destination upload that is gone with a new one, once per request.
    async fn reopen_destination(
        &self,
        session: UploadSession,
        promotion: &Promotion,
        part_copy: &PartCopy,
        blob_key: &Path,
        reopened: &mut bool,
    ) -> Result<Step, OciStorageError> {
        let size = promotion.finished.size;
        if self.holds(blob_key, size).await? {
            self.cleanup_session(session.id()).await;
            return Ok(Step::Done(size));
        }
        if *reopened {
            return Err(OciStorageError::Storage(format!(
                "The upload at {blob_key} is gone again"
            )));
        }
        *reopened = true;
        if let Some(destination) = &promotion.destination {
            self.abort_destination(part_copy, blob_key, destination)
                .await;
        }
        let destination = part_copy.open(blob_key).await?;
        let state = SessionState {
            phase: Phase::Promoting(Promotion {
                destination: Some(destination.clone()),
                ..promotion.clone()
            }),
            ..session.state().clone()
        };
        if let Some(next) = self.commit(&session, state).await? {
            return Ok(Step::Next(Box::new(next)));
        }
        self.abort_destination(part_copy, blob_key, &destination)
            .await;
        Ok(Step::Next(Box::new(self.load(session.id()).await?)))
    }

    /// Finishes a session whose staging object is gone: done when the digest key
    /// holds the blob, and unknown otherwise.
    async fn promoted_elsewhere(
        &self,
        session: &UploadSession,
        finished: &Finished,
        digest: &Digest,
    ) -> Result<Step, OciStorageError> {
        let blob_key = self.storage.blob_key(&session.state().repository, digest);
        if self.holds(&blob_key, finished.size).await? {
            self.cleanup_session(session.id()).await;
            Ok(Step::Done(finished.size))
        } else {
            Err(OciStorageError::UploadNotFound(session.id().to_string()))
        }
    }

    /// Puts a blob of less than one part whole at its digest key, once its bytes
    /// hash to the digest.
    async fn put_whole_blob(
        &self,
        session: &UploadSession,
        digest: &Digest,
        chunks: Vec<Bytes>,
    ) -> Result<(), OciStorageError> {
        let mut hasher = Sha256::new();
        for chunk in &chunks {
            hasher.update(chunk);
        }
        if sha256_digest(hasher)? != *digest {
            return Err(OciStorageError::UploadNotFound(session.id().to_string()));
        }
        let blob_key = self.storage.blob_key(&session.state().repository, digest);
        match self
            .put_checked(&blob_key, PutPayload::from_iter(chunks))
            .await
        {
            Ok(_put) => Ok(()),
            Err(error) => {
                if let OciStorageError::EtagMismatch(_) = error
                    && let Err(e) = self.storage.delete_key(&blob_key).await
                {
                    report_cleanup_error(&self.storage.log, "put_whole_blob: delete", &e);
                }
                Err(error)
            },
        }
    }

    /// Puts an object, and on S3 puts it again once when its `ETag` is not its MD5.
    ///
    /// Returns the object's MD5 and, on S3, its `ETag`.
    async fn put_checked(
        &self,
        key: &Path,
        payload: PutPayload,
    ) -> Result<(String, Option<String>), OciStorageError> {
        let md5 = md5_hex(payload.iter());
        for attempt in 1..=E_TAG_ATTEMPTS {
            let put = self
                .storage
                .store
                .put(key, payload.clone())
                .await
                .map_err(|e| storage_error(&e))?;
            if self.storage.part_copy.is_none() {
                return Ok((md5, None));
            }
            if put.e_tag.as_deref().map(unquoted) == Some(md5.as_str()) {
                return Ok((md5, put.e_tag));
            }
            slog::warn!(self.storage.log, "OCI object ETag is not its MD5"; "key" => %key, "attempt" => attempt);
        }
        Err(OciStorageError::EtagMismatch(format!(
            "{key} was stored with an ETag that is not its MD5"
        )))
    }

    /// Reads a session's tail back, and requires the bytes its newest state committed.
    async fn read_tail(
        &self,
        upload_id: &UploadId,
        key: &Path,
        tail: &Tail,
        length: u64,
    ) -> Result<Bytes, OciStorageError> {
        let unknown = || OciStorageError::UploadNotFound(upload_id.to_string());
        let result = match self.storage.store.get(key).await {
            Ok(result) => result,
            Err(e) if is_not_found(&e) => return Err(unknown()),
            Err(e) => return Err(storage_error(&e)),
        };
        if tail.e_tag.is_some() && result.meta.e_tag != tail.e_tag {
            return Err(unknown());
        }
        let data = result.bytes().await.map_err(|e| storage_error(&e))?;
        if data.len() as u64 != length || md5_hex([&data]) != tail.md5 {
            return Err(unknown());
        }
        Ok(data)
    }

    /// Cancels a session that a failed finalize leaves behind, by its newest state.
    async fn cancel_open(&self, upload_id: &UploadId) {
        if let Err(e) = self.cancel(upload_id).await
            && !matches!(e, OciStorageError::UploadNotFound(_))
        {
            report_cleanup_error(&self.storage.log, "finalize: cancel", &e);
        }
    }

    /// Aborts the staging upload that this request opened, when no committed
    /// state names it. A commit whose answer is lost may name it, so that one stays
    /// for the sweep.
    async fn abort_uncommitted(&self, writer: &PartWriter<'_>) {
        if writer.opened
            && !writer.kept
            && let Some(upload) = &writer.upload
            && let Err(e) = self
                .storage
                .multipart
                .abort_multipart(&writer.key, upload)
                .await
        {
            report_cleanup_error(
                &self.storage.log,
                "upload: abort an uncommitted upload",
                &storage_error(&e),
            );
        }
    }

    /// Deletes a finished session's objects, its states last.
    async fn cleanup_session(&self, upload_id: &UploadId) {
        self.storage
            .sweeper()
            .remove(&self.storage.upload_prefix(upload_id), None)
            .await;
    }

    async fn abort_destination(&self, part_copy: &PartCopy, blob_key: &Path, upload: &MultipartId) {
        if let Err(e) = part_copy.abort(blob_key, upload).await {
            report_cleanup_error(
                &self.storage.log,
                "promote: abort the destination upload",
                &e,
            );
        }
    }

    /// Whether the object at `key` holds `size` bytes.
    async fn holds(&self, key: &Path, size: u64) -> Result<bool, OciStorageError> {
        Ok(self
            .head_object(key)
            .await?
            .is_some_and(|meta| meta.size == size))
    }

    /// The object's metadata, or `None` when it is missing.
    async fn head_object(&self, key: &Path) -> Result<Option<ObjectMeta>, OciStorageError> {
        match self.storage.store.head(key).await {
            Ok(meta) => Ok(Some(meta)),
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(storage_error(&e)),
        }
    }
}

/// Where a finalize stands between its steps.
enum Step {
    /// The session's newest state, from which the finalize goes on
    Next(Box<UploadSession>),
    /// The blob of this many bytes is at its digest key, and the session is gone.
    Done(u64),
}

/// A request body, split into parts of the session's part size as it arrives.
struct PartWriter<'a> {
    uploads: Uploads<'a>,
    upload_id: UploadId,
    /// The staging object's key
    key: Path,
    part_size: u64,
    /// The staging multipart upload, opened at the first full part
    upload: Option<MultipartId>,
    /// Whether this request opened the staging upload
    opened: bool,
    /// Whether a committed state may name the staging upload
    kept: bool,
    /// The `ETag` of each part, committed or uploaded, or `None` while it uploads
    parts: Vec<Option<String>>,
    in_flight: JoinSet<(usize, Result<PartId, OciStorageError>)>,
    /// The first part that failed for good
    failure: Option<OciStorageError>,
    buffer: PartBuffer,
    /// The committed tail, until it is read back into the buffer
    tail: Option<CommittedTail>,
    hasher: Sha256,
    /// The bytes in the session, with this body's
    offset: u64,
    /// The bytes this body held
    received: u64,
}

struct CommittedTail {
    key: Path,
    tail: Tail,
    length: u64,
}

/// The bytes of the part being filled, and their share of the buffer pool.
#[derive(Default)]
struct PartBuffer {
    chunks: Vec<Bytes>,
    length: u64,
    buffer: Option<Buffer>,
}

impl<'a> PartWriter<'a> {
    fn new(uploads: Uploads<'a>, session: &UploadSession, progress: &Progress) -> Self {
        let part_size = session.state().part_size;
        let full = progress.parts.len() as u64 * part_size;
        let tail = progress.tail.as_ref().map(|tail| CommittedTail {
            key: uploads
                .storage
                .upload_tail_key(session.id(), progress.offset),
            tail: tail.clone(),
            length: progress.offset.saturating_sub(full),
        });
        Self {
            uploads,
            upload_id: session.id().clone(),
            key: uploads.storage.upload_data_key(session.id()),
            part_size,
            upload: session.state().upload.clone(),
            opened: false,
            kept: false,
            parts: progress.parts.iter().cloned().map(Some).collect(),
            in_flight: JoinSet::new(),
            failure: None,
            buffer: PartBuffer::default(),
            tail,
            // A session loads only with a hash state this binary reads.
            hasher: progress.hash.hasher().unwrap_or_default(),
            offset: progress.offset,
            received: 0,
        }
    }

    /// Writes the body frame by frame. While a part holds its buffer, the body has
    /// until the part's fill deadline to fill it; past that, the part gives the
    /// buffer back and the request fails.
    async fn write_body<S, E>(&mut self, body: S) -> Result<(), UploadWriteError<E>>
    where
        S: Stream<Item = Result<Bytes, E>>,
    {
        let mut body = pin!(body);
        loop {
            let next = if let Some(fill_by) = self.buffer.buffer.as_ref().map(Buffer::fill_by) {
                let Ok(next) = tokio::time::timeout_at(fill_by, body.next()).await else {
                    drop(self.buffer.take());
                    return Err(OciStorageError::PartStalled(FILL_DEADLINE).into());
                };
                next
            } else {
                body.next().await
            };
            let Some(frame) = next else {
                return Ok(());
            };
            self.write(frame.map_err(UploadWriteError::Body)?).await?;
        }
    }

    async fn write(&mut self, mut frame: Bytes) -> Result<(), OciStorageError> {
        if frame.is_empty() {
            return Ok(());
        }
        self.reap();
        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }
        let size = self.offset + frame.len() as u64;
        if size > self.uploads.storage.max_body_size {
            return Err(OciStorageError::SizeExceeded {
                size,
                max: self.uploads.storage.max_body_size,
            });
        }
        self.read_tail().await?;
        self.hasher.update(&frame);
        self.offset = size;
        self.received += frame.len() as u64;

        while !frame.is_empty() {
            if self.buffer.buffer.is_none() {
                self.buffer.buffer = Some(self.uploads.storage.buffers.take(self.part_size).await?);
            }
            let room = usize::try_from(self.part_size - self.buffer.length).unwrap_or(usize::MAX);
            self.buffer.push(frame.split_to(room.min(frame.len())));
            if self.buffer.length == self.part_size {
                self.start_part().await?;
            }
        }
        Ok(())
    }

    /// Reads the committed tail into the buffer, the first time only.
    async fn read_tail(&mut self) -> Result<(), OciStorageError> {
        let Some(CommittedTail { key, tail, length }) = self.tail.take() else {
            return Ok(());
        };
        let mut buffer = self.uploads.storage.buffers.take(self.part_size).await?;
        let data = self
            .uploads
            .read_tail(&self.upload_id, &key, &tail, length)
            .await?;
        buffer.start_fill();
        self.buffer.buffer = Some(buffer);
        self.buffer.push(data);
        Ok(())
    }

    /// Uploads the full buffer as the next part, once fewer than
    /// [`PARTS_IN_FLIGHT`] parts are in flight.
    async fn start_part(&mut self) -> Result<(), OciStorageError> {
        let (chunks, buffer) = self.buffer.take();
        while self.in_flight.len() >= PARTS_IN_FLIGHT {
            if let Some(joined) = self.in_flight.join_next().await {
                self.record(joined);
            }
        }
        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }
        let upload = if let Some(upload) = &self.upload {
            upload.clone()
        } else {
            let upload = self
                .uploads
                .storage
                .multipart
                .create_multipart(&self.key)
                .await
                .map_err(|e| storage_error(&e))?;
            self.upload = Some(upload.clone());
            self.opened = true;
            upload
        };
        let number = self.parts.len();
        self.parts.push(None);
        let multipart = Arc::clone(&self.uploads.storage.multipart);
        let log = self.uploads.storage.log.clone();
        let key = self.key.clone();
        let check_md5 = self.uploads.storage.part_copy.is_some();
        self.in_flight.spawn(async move {
            let stored = put_part(&multipart, &log, &key, &upload, number, chunks, check_md5).await;
            // The part's memory goes back to the pool, stored or not.
            drop(buffer);
            (number, stored)
        });
        Ok(())
    }

    /// Commits the session's next state, and records whether a committed state
    /// may name the staging upload: when this commit wins, or when its answer is lost.
    async fn commit(
        &mut self,
        session: &UploadSession,
        state: SessionState,
    ) -> Result<Option<UploadSession>, OciStorageError> {
        let committed = self.uploads.commit(session, state).await;
        self.kept = !matches!(committed, Ok(None));
        committed
    }

    /// Records each part upload that has finished.
    fn reap(&mut self) {
        while let Some(joined) = self.in_flight.try_join_next() {
            self.record(joined);
        }
    }

    fn record(&mut self, joined: Result<(usize, Result<PartId, OciStorageError>), JoinError>) {
        match joined {
            Ok((number, Ok(part))) => {
                if let Some(slot) = self.parts.get_mut(number) {
                    *slot = Some(part.content_id);
                }
            },
            Ok((_number, Err(error))) => {
                self.failure.get_or_insert(error);
            },
            Err(error) => {
                self.failure.get_or_insert(OciStorageError::Storage(format!(
                    "A part upload failed: {error}"
                )));
            },
        }
    }

    /// Waits for every part in flight, and returns the `ETag` of each part, or the first failure.
    ///
    /// A failed part never cancels the others: each call in flight finishes, so
    /// each records its answer.
    async fn settle(&mut self) -> Result<Vec<String>, OciStorageError> {
        while let Some(joined) = self.in_flight.join_next().await {
            self.record(joined);
        }
        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }
        self.parts
            .iter()
            .cloned()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| OciStorageError::Storage("A part upload never finished".to_owned()))
    }

    /// Stores the bytes past the last full part as the tail for the new offset.
    async fn store_tail(&mut self) -> Result<Option<Tail>, OciStorageError> {
        let (chunks, buffer) = self.buffer.take();
        if chunks.is_empty() {
            return Ok(None);
        }
        let key = self
            .uploads
            .storage
            .upload_tail_key(&self.upload_id, self.offset);
        let (md5, e_tag) = self
            .uploads
            .put_checked(&key, PutPayload::from_iter(chunks))
            .await?;
        drop(buffer);
        Ok(Some(Tail { md5, e_tag }))
    }
}

impl PartBuffer {
    fn push(&mut self, chunk: Bytes) {
        self.length += chunk.len() as u64;
        self.chunks.push(chunk);
    }

    /// Takes the bytes and their buffer, and leaves the buffer empty.
    fn take(&mut self) -> (Vec<Bytes>, Option<Buffer>) {
        let Self {
            chunks,
            length: _,
            buffer,
        } = std::mem::take(self);
        (chunks, buffer)
    }
}

/// Uploads one part, and on S3 uploads it again once when its `ETag` is not its MD5.
async fn put_part(
    multipart: &RecordingMultipart,
    log: &Logger,
    key: &Path,
    upload: &MultipartId,
    number: usize,
    chunks: Vec<Bytes>,
    check_md5: bool,
) -> Result<PartId, OciStorageError> {
    let payload = PutPayload::from_iter(chunks);
    let md5 = check_md5.then(|| md5_hex(payload.iter()));
    for attempt in 1..=E_TAG_ATTEMPTS {
        let part = multipart
            .put_part(key, upload, number, payload.clone())
            .await
            .map_err(|e| storage_error(&e))?;
        if md5
            .as_deref()
            .is_none_or(|md5| unquoted(&part.content_id) == md5)
        {
            return Ok(part);
        }
        slog::warn!(log, "OCI part ETag is not its MD5"; "part" => number + 1, "attempt" => attempt);
    }
    Err(OciStorageError::EtagMismatch(format!(
        "Part {} of {key} was stored with an ETag that is not its MD5",
        number + 1
    )))
}

/// Requires a completed staging object's `ETag` to be the multipart `ETag` of its parts.
fn check_completed_e_tag(
    staging: &Path,
    e_tag: Option<&str>,
    parts: &[String],
) -> Result<(), OciStorageError> {
    let parts: Vec<PartId> = parts
        .iter()
        .map(|e_tag| PartId {
            content_id: e_tag.clone(),
        })
        .collect();
    let expected = multipart_e_tag(&parts);
    if expected.is_some() && e_tag.map(unquoted) == expected.as_deref() {
        Ok(())
    } else {
        Err(OciStorageError::EtagMismatch(format!(
            "{staging} has ETag {}, not its parts' ETag {}",
            e_tag.unwrap_or_default(),
            expected.unwrap_or_default()
        )))
    }
}

/// Whether a completion failed as one does once the upload is gone: completed
/// before, as after a lost answer, or aborted.
fn is_upload_gone(error: &object_store::Error) -> bool {
    is_not_found(error) || is_no_such_upload(error)
}

/// Requires a resumed session to be the upload of `digest`, and leaves it as it is when not.
fn check_digest(finished: &Finished, digest: &Digest) -> Result<(), OciStorageError> {
    if finished.digest == digest.as_str() {
        Ok(())
    } else {
        Err(OciStorageError::DigestMismatch {
            expected: digest.to_string(),
            actual: finished.digest.clone(),
        })
    }
}

/// Whether a completion was refused for a part that is not the one its `ETag`
/// names, as when another request replaced it: R2's 400 `InvalidPart`, or the
/// file store's refusal.
fn is_part_refused(error: &object_store::Error) -> bool {
    matches!(error, object_store::Error::Precondition { .. }) || is_invalid_part(error)
}

fn sha256_digest(hasher: Sha256) -> Result<Digest, OciStorageError> {
    Digest::sha256(&hex::encode(hasher.finalize()))
        .map_err(|e| OciStorageError::InvalidContent(e.to_string()))
}

/// How a failed finalize ended, for its count.
fn finalize_outcome(error: &OciStorageError) -> FinalizeOutcome {
    match error {
        OciStorageError::DigestMismatch { .. } => FinalizeOutcome::DigestMismatch,
        OciStorageError::EtagMismatch(_) => FinalizeOutcome::EtagMismatch,
        OciStorageError::Storage(_)
        | OciStorageError::UploadNotFound(_)
        | OciStorageError::BlobNotFound(_)
        | OciStorageError::ManifestNotFound(_)
        | OciStorageError::InvalidContent(_)
        | OciStorageError::BlobUploadInvalidContent(_)
        | OciStorageError::Config(_)
        | OciStorageError::Json(_)
        | OciStorageError::SizeExceeded { .. }
        | OciStorageError::PartStalled(_) => FinalizeOutcome::Error,
    }
}

fn state_json(state: &SessionState) -> Result<Vec<u8>, OciStorageError> {
    serde_json::to_vec(state).map_err(|e| OciStorageError::Json(e.to_string()))
}
