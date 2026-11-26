//! Upload sessions through crashes, races, and the sweep.
//!
//! [`Faulty`] wraps the in-memory store or the file store and, once a chosen call
//! of a chosen kind arrives, fails it and every call after it, as a crash there
//! would. A crash "before" a call fails it unsent; a crash "after" one sends it and
//! loses its answer.

use std::convert::Infallible;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bencher_json::DateTime;
use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt as _};
use object_store::memory::InMemory;
use object_store::multipart::{MultipartStore, PartId};
use object_store::path::Path;
use object_store::{
    CopyOptions, GetOptions, GetResult, ListResult, MultipartId, MultipartUpload, ObjectMeta,
    ObjectStore, ObjectStoreExt as _, PutMode, PutMultipartOptions, PutOptions, PutPayload,
    PutResult, RenameOptions, Result,
};
use slog::Logger;
use tokio::sync::{Barrier, Semaphore};

use super::session::{Phase, Promotion, UploadSession};
use super::{
    Appended, BufferPool, Clock, DEFAULT_MAX_BODY_SIZE, Digest, FileMultipart, MAX_PARTS,
    OciStorage, OciStorageError, ProjectUuid, RecordingMultipart, RecordingStore, Sweep, UploadId,
    UploadWriteError, file_store,
};
use crate::buffers::FILL_DEADLINE;

/// The part size of every test session
const PART: usize = 16;

/// Half a part
const HALF_PART: usize = 8;

const PROJECT: &str = "00000000-0000-0000-0000-000000000001";

// ==================== The crash table ====================
//
// One test per row and backend for the retried request, and one for the sweep.

/// A PATCH that crashes before its commit leaves its parts and tail uncommitted;
/// the client resends from the committed offset, and the resent parts replace them.
async fn a_patch_lost_before_its_commit_resends_from_the_committed_offset(backend: Backend) {
    let (fixture, upload_id, blob) = crash_mid_patch(backend).await;

    let committed = fixture.storage.get_upload_size(&upload_id).await.unwrap();
    assert_eq!(
        committed, FIRST_PATCH as u64,
        "the lost PATCH added nothing"
    );
    let appended = patch(&fixture.storage, &upload_id, &blob[FIRST_PATCH..]).await;
    assert!(appended.committed);
    assert_eq!(appended.offset, blob.len() as u64);

    finish_and_check(&fixture, &upload_id, &blob).await;
}

/// The sweep aborts the saved staging upload of a session left mid-PATCH, and
/// deletes its tails and states.
async fn a_patch_lost_before_its_commit_is_swept(backend: Backend) {
    let (fixture, upload_id, _blob) = crash_mid_patch(backend).await;
    let staging = staging_upload(&fixture, &upload_id).await;

    sweep_stale(&fixture).await;

    assert_swept(&fixture, &upload_id, &[staging]).await;
}

/// A finalize that crashes after it commits `Completing` leaves the staging upload
/// open; a retry completes it.
async fn a_finalize_lost_before_completion_completes_on_retry(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, |call| {
        matches!(call, Call::Complete(_))
    })
    .await;

    finish_and_check(&fixture, &upload_id, &blob).await;
}

async fn a_finalize_lost_before_completion_is_swept(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, |call| {
        matches!(call, Call::Complete(_))
    })
    .await;
    let staging = staging_upload(&fixture, &upload_id).await;

    sweep_stale(&fixture).await;

    assert_swept(&fixture, &upload_id, &[staging]).await;
    assert_no_blob(&fixture, &blob).await;
}

/// A completion whose answer is lost completed the staging upload, so its retry
/// finds the upload gone and the staging object whole.
async fn a_lost_completion_answer_is_found_on_retry(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::After, |call| {
        matches!(call, Call::Complete(_))
    })
    .await;
    assert!(
        fixture
            .raw_exists(&fixture.storage.upload_data_key(&upload_id))
            .await,
        "the staging object is complete"
    );

    finish_and_check(&fixture, &upload_id, &blob).await;
}

async fn a_lost_completion_answer_is_swept(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::After, |call| {
        matches!(call, Call::Complete(_))
    })
    .await;
    let staging = staging_upload(&fixture, &upload_id).await;

    sweep_stale(&fixture).await;

    assert_swept(&fixture, &upload_id, &[staging]).await;
    assert_no_blob(&fixture, &blob).await;
}

/// A finalize that crashes after it commits `Promoting` and before the blob moves
/// promotes on retry.
async fn a_finalize_lost_before_the_promote_promotes_on_retry(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, |call| {
        matches!(call, Call::Rename(..))
    })
    .await;

    finish_and_check(&fixture, &upload_id, &blob).await;
}

async fn a_finalize_lost_before_the_promote_is_swept(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, |call| {
        matches!(call, Call::Rename(..))
    })
    .await;
    let staging = staging_upload(&fixture, &upload_id).await;

    sweep_stale(&fixture).await;

    assert_swept(&fixture, &upload_id, &[staging]).await;
    assert_no_blob(&fixture, &blob).await;
}

/// A finalize that crashes after the blob reaches its digest key, before its
/// cleanup, finds the blob on retry and only cleans up.
async fn a_finalize_lost_before_cleanup_finishes_on_retry(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::After, |call| {
        matches!(call, Call::Rename(..))
    })
    .await;
    assert_blob(&fixture, &blob).await;
    fixture.faults.clear_calls();

    finish_and_check(&fixture, &upload_id, &blob).await;
    assert!(
        !fixture
            .faults
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Rename(..) | Call::Copy(..))),
        "the blob does not move again"
    );
}

async fn a_finalize_lost_before_cleanup_is_swept(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::After, |call| {
        matches!(call, Call::Rename(..))
    })
    .await;
    let staging = staging_upload(&fixture, &upload_id).await;

    sweep_stale(&fixture).await;

    assert_swept(&fixture, &upload_id, &[staging]).await;
    assert_blob(&fixture, &blob).await;
}

/// Two sessions upload one digest at once; both finish, and the digest key holds its bytes.
async fn two_uploads_of_one_digest_both_finish(backend: Backend) {
    let fixture = fixture(backend);
    let blob = blob_bytes(2 * PART + HALF_PART);
    let first = start(&fixture.storage).await;
    let second = start(&fixture.storage).await;
    for upload_id in [&first, &second] {
        patch(&fixture.storage, upload_id, &blob).await;
    }
    fixture.faults.hold_commits(2);

    let digest = Digest::from_sha256_bytes(&blob);
    let (finished_first, finished_second) = tokio::join!(
        fixture.storage.complete_upload(&first, &digest),
        fixture.storage.complete_upload(&second, &digest),
    );

    finished_first.unwrap();
    finished_second.unwrap();
    assert_blob(&fixture, &blob).await;
    for upload_id in [&first, &second] {
        assert_session_gone(&fixture, upload_id).await;
    }
}

// ==================== Races ====================

/// Two PATCH requests from one committed state, with bodies of one length and
/// other bytes, where the loser writes after the winner commits, as a slow loser
/// does: the loser's bytes replace the winner's, so the finalize fails, and
/// nothing reaches either digest key.
///
/// `committed` is the length of the first PATCH, which opens the staging upload.
async fn racing_patches_never_store_wrong_bytes(
    backend: Backend,
    committed: usize,
    length: usize,
) -> OciStorageError {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let first = blob_bytes(committed);
    patch(&fixture.storage, &upload_id, &first).await;
    let winner = blob_bytes(committed + length)[committed..].to_vec();
    let loser: Vec<u8> = winner.iter().map(|byte| byte ^ 0xff).collect();
    let winner_session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    let loser_session = fixture.storage.uploads().load(&upload_id).await.unwrap();

    let won = append(&fixture.storage, winner_session, &winner).await;
    let lost = append(&fixture.storage, loser_session, &loser).await;

    assert!(won.committed && !lost.committed, "the first to commit wins");
    assert_eq!(
        lost.offset, won.offset,
        "the loser learns the committed range"
    );
    let winning_blob = [first.as_slice(), &winner].concat();
    let losing_blob = [first.as_slice(), &loser].concat();
    let error = fixture
        .storage
        .complete_upload(&upload_id, &Digest::from_sha256_bytes(&winning_blob))
        .await
        .unwrap_err();
    assert_no_blob(&fixture, &winning_blob).await;
    assert_no_blob(&fixture, &losing_blob).await;
    error
}

/// The loser replaces the winner's tail, which becomes the last part.
async fn racing_tails_never_store_wrong_bytes(backend: Backend) {
    let error = racing_patches_never_store_wrong_bytes(backend, PART + 4, 4).await;
    assert!(
        matches!(error, OciStorageError::UploadNotFound(_)),
        "{error}"
    );
}

/// The PATCH that loses the commit learns the range the winner committed, not its own.
async fn a_patch_that_loses_the_commit_learns_the_committed_range(backend: Backend) {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let (short, long) = (blob_bytes(3), blob_bytes(PART + 5));
    fixture.faults.hold_commits(2);

    let (one, other) = tokio::join!(
        patch(&fixture.storage, &upload_id, &short),
        patch(&fixture.storage, &upload_id, &long),
    );

    assert_ne!(one.committed, other.committed, "exactly one commit wins");
    let committed = if one.committed {
        one.offset
    } else {
        other.offset
    };
    assert_eq!(one.offset, committed);
    assert_eq!(other.offset, committed);
    assert_eq!(
        fixture.storage.get_upload_size(&upload_id).await.unwrap(),
        committed
    );
}

/// A tail that changes after its commit, as the losing PATCH of a race can change
/// it, makes the session unknown: neither a PATCH nor a finalize uses it.
async fn a_tail_changed_after_its_commit_makes_the_session_unknown(backend: Backend) {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(PART + HALF_PART);
    patch(&fixture.storage, &upload_id, &blob).await;
    let tail = fixture
        .storage
        .upload_tail_key(&upload_id, blob.len() as u64);
    let changed: Vec<u8> = blob[PART..].iter().map(|byte| byte ^ 0xff).collect();
    fixture
        .raw
        .put(&tail, Bytes::from(changed).into())
        .await
        .unwrap();

    let appended = fixture
        .storage
        .append_upload(&upload_id, Bytes::from_static(b"more"))
        .await
        .unwrap_err();
    assert!(
        matches!(appended, OciStorageError::UploadNotFound(_)),
        "{appended}"
    );
    let finished = fixture
        .storage
        .complete_upload(&upload_id, &Digest::from_sha256_bytes(&blob))
        .await
        .unwrap_err();
    assert!(
        matches!(finished, OciStorageError::UploadNotFound(_)),
        "{finished}"
    );
    assert_no_blob(&fixture, &blob).await;
}

/// A client retries a PUT while the first still finishes: one answers 201, the
/// other 201 or an error it may retry, and the winner keeps every state it needs.
async fn overlapping_finalizes_never_break_the_winner(backend: Backend) {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(2 * PART + HALF_PART);
    patch(&fixture.storage, &upload_id, &blob).await;
    fixture.faults.hold_commits(2);

    let digest = Digest::from_sha256_bytes(&blob);
    let (first, second) = tokio::join!(
        fixture.storage.complete_upload(&upload_id, &digest),
        fixture.storage.complete_upload(&upload_id, &digest),
    );

    assert!(first.is_ok() || second.is_ok(), "{first:?} {second:?}");
    for finished in [first, second] {
        if let Err(error) = finished {
            assert!(
                matches!(
                    error,
                    OciStorageError::UploadNotFound(_) | OciStorageError::Storage(_)
                ),
                "an error a client retries: {error}"
            );
        }
    }
    assert_blob(&fixture, &blob).await;
    assert_session_gone(&fixture, &upload_id).await;
}

/// A PUT that resumes a session past its digest check with another digest stores
/// nothing under either digest key, and leaves the session to its own digest.
async fn resumed_with_another_digest_stores_nothing<F>(backend: Backend, matches: F)
where
    F: Fn(&Call) -> bool + Send + Sync + 'static,
{
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, matches).await;
    let other = blob_bytes(blob.len() + 1);

    let error = fixture
        .storage
        .complete_upload(&upload_id, &Digest::from_sha256_bytes(&other))
        .await
        .unwrap_err();

    assert!(
        matches!(error, OciStorageError::DigestMismatch { .. }),
        "{error}"
    );
    assert_no_blob(&fixture, &other).await;
    assert_no_blob(&fixture, &blob).await;
    finish_and_check(&fixture, &upload_id, &blob).await;
}

async fn a_resumed_completing_session_with_another_digest_stores_nothing(backend: Backend) {
    resumed_with_another_digest_stores_nothing(backend, |call| matches!(call, Call::Complete(_)))
        .await;
}

async fn a_resumed_promoting_session_with_another_digest_stores_nothing(backend: Backend) {
    resumed_with_another_digest_stores_nothing(backend, |call| matches!(call, Call::Rename(..)))
        .await;
}

/// A session whose staging upload and object are gone finishes when the digest key
/// already holds its blob, as another upload of the digest leaves it.
async fn a_gone_staging_object_with_the_blob_in_place_finishes(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, |call| {
        matches!(call, Call::Complete(_))
    })
    .await;
    let other = start(&fixture.storage).await;
    patch(&fixture.storage, &other, &blob).await;
    finish_and_check(&fixture, &other, &blob).await;
    let staging = staging_upload(&fixture, &upload_id).await;
    fixture
        .storage
        .multipart
        .abort_multipart(&fixture.storage.upload_data_key(&upload_id), &staging)
        .await
        .unwrap();

    finish_and_check(&fixture, &upload_id, &blob).await;
}

/// A first PATCH whose commit lands but whose answer is lost keeps the upload
/// it opened, which the committed state names, and the retried finalize uses it.
async fn a_patch_whose_commit_answer_is_lost_keeps_its_upload(backend: Backend) {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(2 * PART + HALF_PART);
    fixture.faults.crash(Crash::AnswerLost, 1, Call::is_commit);
    fixture
        .storage
        .append_upload(&upload_id, Bytes::copy_from_slice(&blob))
        .await
        .unwrap_err();

    assert_eq!(
        fixture.storage.get_upload_size(&upload_id).await.unwrap(),
        blob.len() as u64,
        "the commit landed"
    );
    finish_and_check(&fixture, &upload_id, &blob).await;
}

// ==================== Cancel ====================

/// A cancel reads the newest state: a session past its digest check stays for its retry.
async fn a_cancel_leaves_a_finishing_session_for_its_retry(backend: Backend) {
    let (fixture, upload_id, blob) = crash_in_finalize(backend, Crash::Before, |call| {
        matches!(call, Call::Complete(_))
    })
    .await;

    fixture.storage.cancel_upload(&upload_id).await.unwrap();

    finish_and_check(&fixture, &upload_id, &blob).await;
}

/// A cancel of an open session aborts its staging upload and removes it.
async fn a_cancel_of_an_open_session_aborts_and_removes_it(backend: Backend) {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    patch(&fixture.storage, &upload_id, &blob_bytes(PART + 1)).await;
    let staging = staging_upload(&fixture, &upload_id).await;

    fixture.storage.cancel_upload(&upload_id).await.unwrap();

    assert_swept(&fixture, &upload_id, &[staging]).await;
}

macro_rules! on_both_backends {
    ($($name:ident),* $(,)?) => {
        mod in_memory {
            $(
                #[tokio::test]
                async fn $name() {
                    super::$name(super::Backend::Memory).await;
                }
            )*
        }

        mod on_disk {
            $(
                #[tokio::test]
                async fn $name() {
                    super::$name(super::Backend::File).await;
                }
            )*
        }
    };
}

on_both_backends!(
    a_patch_lost_before_its_commit_resends_from_the_committed_offset,
    a_patch_lost_before_its_commit_is_swept,
    a_finalize_lost_before_completion_completes_on_retry,
    a_finalize_lost_before_completion_is_swept,
    a_lost_completion_answer_is_found_on_retry,
    a_lost_completion_answer_is_swept,
    a_finalize_lost_before_the_promote_promotes_on_retry,
    a_finalize_lost_before_the_promote_is_swept,
    a_finalize_lost_before_cleanup_finishes_on_retry,
    a_finalize_lost_before_cleanup_is_swept,
    two_uploads_of_one_digest_both_finish,
    racing_tails_never_store_wrong_bytes,
    a_patch_that_loses_the_commit_learns_the_committed_range,
    a_resumed_completing_session_with_another_digest_stores_nothing,
    a_resumed_promoting_session_with_another_digest_stores_nothing,
    a_gone_staging_object_with_the_blob_in_place_finishes,
    a_patch_whose_commit_answer_is_lost_keeps_its_upload,
    a_tail_changed_after_its_commit_makes_the_session_unknown,
    overlapping_finalizes_never_break_the_winner,
    a_cancel_leaves_a_finishing_session_for_its_retry,
    a_cancel_of_an_open_session_aborts_and_removes_it,
);

/// On the file store a PATCH that replaces the winner's part fails the
/// completion, since each part must hold the bytes of its `ETag`, and the client
/// starts the blob again.
#[tokio::test]
async fn racing_parts_never_store_wrong_bytes_on_disk() {
    let error = racing_patches_never_store_wrong_bytes(Backend::File, PART, PART).await;
    assert!(
        matches!(error, OciStorageError::UploadNotFound(_)),
        "{error}"
    );
}

// ==================== Sessions ====================

/// A blob that never fills a part is one put straight to its digest key.
#[tokio::test]
async fn a_blob_under_one_part_is_one_put_to_its_digest_key() {
    let fixture = fixture(Backend::Memory);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(PART - 1);
    patch(&fixture.storage, &upload_id, &blob).await;
    fixture.faults.clear_calls();

    finish_and_check(&fixture, &upload_id, &blob).await;

    let calls = fixture.faults.calls();
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            Call::Create(_) | Call::PutPart(..) | Call::Complete(_) | Call::Rename(..)
        )),
        "no multipart upload: {calls:?}"
    );
    let blob_key = Call::Put(fixture.blob_key(&blob).to_string());
    assert_eq!(
        calls.iter().filter(|call| **call == blob_key).count(),
        1,
        "{calls:?}"
    );
}

/// A session whose hash state another build wrote is unknown, so the client
/// starts the blob again.
#[tokio::test]
async fn a_hash_state_this_binary_cannot_read_makes_the_session_unknown() {
    let fixture = fixture(Backend::Memory);
    let upload_id = start(&fixture.storage).await;
    patch(&fixture.storage, &upload_id, b"some bytes").await;
    let session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    let key = fixture
        .storage
        .upload_state_generation_key(&upload_id, session.generation());
    let mut state: serde_json::Value =
        serde_json::from_slice(&fixture.raw.get(&key).await.unwrap().bytes().await.unwrap())
            .unwrap();
    state["phase"]["hash"]["tag"] = "sha256/another-build".into();
    fixture
        .raw
        .put(&key, serde_json::to_vec(&state).unwrap().into())
        .await
        .unwrap();

    for error in [
        fixture
            .storage
            .append_upload(&upload_id, Bytes::from_static(b"more"))
            .await
            .unwrap_err(),
        fixture
            .storage
            .complete_upload(&upload_id, &Digest::from_sha256_bytes(b"some bytes"))
            .await
            .unwrap_err(),
    ] {
        assert!(
            matches!(error, OciStorageError::UploadNotFound(_)),
            "{error}"
        );
    }
}

/// A session in the old layout is unknown to a PATCH and a PUT, and the sweep removes it.
#[tokio::test]
async fn an_old_layout_session_is_unknown_and_swept() {
    let fixture = fixture(Backend::Memory);
    let upload_id = UploadId::new();
    let prefix = fixture.storage.upload_prefix(&upload_id);
    fixture
        .raw
        .put(
            &prefix.clone().join("state.json"),
            format!(r#"{{"repository":"{PROJECT}","created_at":0}}"#).into(),
        )
        .await
        .unwrap();
    fixture
        .raw
        .put(
            &prefix
                .clone()
                .join("chunks")
                .join("00000000000000000001_chunk"),
            Bytes::from_static(b"old bytes").into(),
        )
        .await
        .unwrap();

    let appended = fixture
        .storage
        .append_upload(&upload_id, Bytes::from_static(b"new bytes"))
        .await
        .unwrap_err();
    let finished = fixture
        .storage
        .complete_upload(&upload_id, &Digest::from_sha256_bytes(b"old bytes"))
        .await
        .unwrap_err();
    for error in [appended, finished] {
        assert!(
            matches!(error, OciStorageError::UploadNotFound(_)),
            "{error}"
        );
    }

    sweep_stale(&fixture).await;
    assert_session_gone(&fixture, &upload_id).await;
}

/// The sweep aborts the staging and the destination uploads by their saved IDs,
/// then deletes the staging object and the tails, and the states last.
#[tokio::test]
async fn the_sweep_aborts_both_saved_uploads_and_deletes_states_last() {
    let (fixture, upload_id, blob) = crash_in_finalize(Backend::Memory, Crash::Before, |call| {
        matches!(call, Call::Rename(..))
    })
    .await;
    let session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    let Phase::Promoting(promotion) = session.state().phase.clone() else {
        panic!("{:?}", session.state().phase);
    };
    let destination = "a-destination-upload".to_owned();
    let mut state = session.state().clone();
    state.phase = Phase::Promoting(Promotion {
        destination: Some(destination.clone()),
        ..promotion
    });
    let staging = state.upload.clone().unwrap();
    fixture
        .storage
        .uploads()
        .commit(&session, state)
        .await
        .unwrap()
        .unwrap();
    fixture.faults.clear_calls();

    sweep_stale(&fixture).await;

    let calls = fixture.faults.calls();
    let aborts: Vec<&Call> = calls
        .iter()
        .filter(|call| matches!(call, Call::Abort(..)))
        .collect();
    assert_eq!(
        aborts,
        [
            &Call::Abort(
                fixture.storage.upload_data_key(&upload_id).to_string(),
                staging
            ),
            &Call::Abort(fixture.blob_key(&blob).to_string(), destination),
        ]
    );
    let deletes: Vec<String> = calls
        .iter()
        .filter_map(|call| {
            if let Call::Delete(key) = call {
                Some(key.clone())
            } else {
                None
            }
        })
        .collect();
    let states = fixture.storage.upload_states_prefix(&upload_id).to_string();
    let first_state = deletes
        .iter()
        .position(|key| key.starts_with(&states))
        .unwrap();
    assert!(
        deletes[..first_state].contains(&fixture.storage.upload_data_key(&upload_id).to_string()),
        "the staging object goes before any state: {deletes:?}"
    );
    assert!(
        deletes[first_state..]
            .iter()
            .all(|key| key.starts_with(&states)),
        "the states go last: {deletes:?}"
    );
    assert!(
        deletes[first_state..].is_sorted(),
        "the states go in generation order: {deletes:?}"
    );
    assert_session_gone(&fixture, &upload_id).await;
}

/// A sweep that cannot delete the staging object keeps every state, so the next
/// sweep still finds the session and deletes the rest.
#[tokio::test]
async fn a_sweep_that_cannot_delete_the_staging_object_keeps_the_states() {
    let (fixture, upload_id, _blob) = crash_in_finalize(Backend::Memory, Crash::Before, |call| {
        matches!(call, Call::Rename(..))
    })
    .await;
    let staging = Call::Delete(fixture.storage.upload_data_key(&upload_id).to_string());
    fixture
        .faults
        .crash(Crash::Once, 1, move |call| *call == staging);

    sweep_stale(&fixture).await;

    let states = fixture.storage.upload_states_prefix(&upload_id);
    assert!(
        !fixture.raw_objects(&states).await.is_empty(),
        "the states stay"
    );
    sweep_stale(&fixture).await;
    assert_session_gone(&fixture, &upload_id).await;
}

/// A PATCH that finds the buffer pool empty waits for a part to finish, and then
/// finishes: none fails for want of memory.
#[tokio::test]
async fn a_patch_waits_for_a_buffer_and_finishes_once_one_frees() {
    let fixture = fixture(Backend::Memory);
    let first = start(&fixture.storage).await;
    let second = start(&fixture.storage).await;
    let gate = fixture.faults.hold_parts();

    let (filling, waiting) = (blob_bytes(PART), blob_bytes(HALF_PART));
    let mut filling = std::pin::pin!(patch(&fixture.storage, &first, &filling));
    let mut waiting = std::pin::pin!(patch(&fixture.storage, &second, &waiting));

    // The first fills the only buffer and holds it while its part uploads.
    let held = tokio::time::timeout(
        Duration::from_millis(200),
        futures::future::select(&mut filling, &mut waiting),
    )
    .await;
    assert!(
        held.is_err(),
        "the second PATCH waits while the first holds the only buffer"
    );

    gate.add_permits(1);
    let (filled, waited) = tokio::time::timeout(
        Duration::from_secs(10),
        futures::future::join(filling, waiting),
    )
    .await
    .expect("a part that finishes gives its buffer back");
    assert!(filled.committed && waited.committed);
    assert_eq!(waited.offset, HALF_PART as u64);
}

/// A part whose upload fails gives its buffer back, so the next request finishes.
#[tokio::test]
async fn a_failed_part_gives_its_buffer_back() {
    let fixture = fixture(Backend::Memory);
    let failing = start(&fixture.storage).await;
    let next = start(&fixture.storage).await;
    fixture
        .faults
        .crash(Crash::Before, 1, |call| matches!(call, Call::PutPart(..)));
    fixture
        .storage
        .append_upload(&failing, blob_bytes(PART).into())
        .await
        .unwrap_err();
    fixture.faults.recover();

    let appended = tokio::time::timeout(
        Duration::from_secs(5),
        patch(&fixture.storage, &next, &blob_bytes(HALF_PART)),
    )
    .await
    .expect("the failed part's buffer is back in the pool");

    assert!(appended.committed);
}

// ==================== The fill deadline ====================

/// A part that takes its buffer and does not fill within the deadline fails its
/// request with an error that clients retry, commits nothing, and gives its buffer
/// back, so the request that waits for the buffer finishes.
#[tokio::test(start_paused = true)]
async fn a_part_that_stalls_past_the_deadline_fails_and_gives_its_buffer_back() {
    let fixture = fixture(Backend::Memory);
    let stalled = start(&fixture.storage).await;
    let waiting = start(&fixture.storage).await;
    let session = fixture.storage.uploads().load(&stalled).await.unwrap();
    let stalling = stream::iter([Ok::<_, Infallible>(Bytes::from(blob_bytes(HALF_PART)))])
        .chain(stream::pending());
    let waiting_bytes = blob_bytes(HALF_PART);
    let began = tokio::time::Instant::now();

    let (failed, waited) = tokio::time::timeout(Duration::from_mins(10), async {
        tokio::join!(
            fixture.storage.append_upload_body(session, stalling),
            patch(&fixture.storage, &waiting, &waiting_bytes),
        )
    })
    .await
    .expect("the stalled part fails at its deadline");

    let Err(UploadWriteError::Storage(error)) = failed else {
        panic!("{failed:?}");
    };
    assert_eq!(
        error.status_code(),
        http::StatusCode::REQUEST_TIMEOUT,
        "{error}"
    );
    assert!(began.elapsed() >= FILL_DEADLINE);
    assert!(waited.committed, "the waiting request gets the buffer");
    assert_eq!(
        fixture.storage.get_upload_size(&stalled).await.unwrap(),
        0,
        "the stalled request commits nothing"
    );
}

/// Parts that fill slowly but each within the deadline succeed, though the whole
/// body takes longer than one deadline.
#[tokio::test(start_paused = true)]
async fn parts_that_fill_slowly_within_the_deadline_succeed() {
    let fixture = fixture(Backend::Memory);
    let upload_id = start(&fixture.storage).await;
    let session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    let frames: Vec<Bytes> = blob_bytes(2 * PART)
        .chunks(HALF_PART)
        .map(Bytes::copy_from_slice)
        .collect();
    let slow = stream::iter(frames).then(|frame| async move {
        tokio::time::sleep(Duration::from_mins(2)).await;
        Ok::<_, Infallible>(frame)
    });
    let began = tokio::time::Instant::now();

    let appended = fixture
        .storage
        .append_upload_body(session, slow)
        .await
        .unwrap();

    assert!(appended.committed);
    assert_eq!(appended.offset, 2 * PART as u64);
    assert!(
        began.elapsed() > FILL_DEADLINE,
        "the whole body outlasts one deadline"
    );
}

/// The time a full part spends on its way to storage, and the time a request
/// waits for a buffer, count against no deadline: a part's deadline starts when it
/// takes its buffer.
#[tokio::test(start_paused = true)]
async fn a_slow_part_upload_counts_against_no_deadline() {
    let fixture = fixture(Backend::Memory);
    let upload_id = start(&fixture.storage).await;
    let session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    let gate = fixture.faults.hold_parts();
    let frames: Vec<Bytes> = blob_bytes(2 * PART + HALF_PART)
        .chunks(HALF_PART)
        .map(Bytes::copy_from_slice)
        .collect();
    let body = stream::iter(frames).then(|frame| async move {
        tokio::time::sleep(Duration::from_mins(1)).await;
        Ok::<_, Infallible>(frame)
    });
    let release = async {
        tokio::time::sleep(Duration::from_mins(10)).await;
        gate.add_permits(2);
    };

    let (appended, ()) = tokio::join!(fixture.storage.append_upload_body(session, body), release);

    let appended = appended.unwrap();
    assert!(appended.committed);
    assert_eq!(appended.offset, (2 * PART + HALF_PART) as u64);
}

/// A part that starts from a committed tail reads the tail back from storage
/// first. The deadline covers only the fill from the request body, so it starts
/// once the read-back ends.
#[tokio::test(start_paused = true)]
async fn a_slow_tail_read_back_counts_against_no_deadline() {
    let fixture = fixture(Backend::Memory);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(PART);
    assert!(
        patch(&fixture.storage, &upload_id, &blob[..HALF_PART])
            .await
            .committed
    );
    let gate = fixture.faults.hold_tail_reads();
    let session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    // The first frame takes the buffer and reads the tail back; the second fills the part.
    let (first, rest) = blob[HALF_PART..].split_at(1);
    let frames = [Bytes::copy_from_slice(first), Bytes::copy_from_slice(rest)];
    let body = stream::iter(frames)
        .enumerate()
        .then(|(index, frame)| async move {
            if index > 0 {
                tokio::time::sleep(Duration::from_mins(2)).await;
            }
            Ok::<_, Infallible>(frame)
        });
    let release = async {
        tokio::time::sleep(Duration::from_mins(4)).await;
        gate.add_permits(1);
    };
    let began = tokio::time::Instant::now();

    let (appended, ()) = tokio::time::timeout(Duration::from_mins(20), async {
        tokio::join!(fixture.storage.append_upload_body(session, body), release)
    })
    .await
    .expect("the read-back is released");

    let appended = appended.unwrap();
    assert!(appended.committed);
    assert_eq!(appended.offset, PART as u64);
    assert!(
        began.elapsed() > FILL_DEADLINE,
        "the part fills later than one deadline after it took its buffer"
    );
}

/// A part whose frames each come well within the deadline, but which does not
/// fill within it, fails: the deadline is for the part, not for each frame.
#[tokio::test(start_paused = true)]
async fn a_part_that_trickles_past_the_deadline_fails() {
    let fixture = fixture(Backend::Memory);
    let upload_id = start(&fixture.storage).await;
    let session = fixture.storage.uploads().load(&upload_id).await.unwrap();
    let frames: Vec<Bytes> = blob_bytes(PART)
        .chunks(2)
        .map(Bytes::copy_from_slice)
        .collect();
    let trickle = stream::iter(frames).then(|frame| async move {
        tokio::time::sleep(Duration::from_mins(1)).await;
        Ok::<_, Infallible>(frame)
    });

    let failed = tokio::time::timeout(
        Duration::from_mins(20),
        fixture.storage.append_upload_body(session, trickle),
    )
    .await
    .expect("the part fails at its deadline");

    let Err(UploadWriteError::Storage(error)) = failed else {
        panic!("{failed:?}");
    };
    assert_eq!(
        error.status_code(),
        http::StatusCode::REQUEST_TIMEOUT,
        "{error}"
    );
    assert_eq!(
        fixture.storage.get_upload_size(&upload_id).await.unwrap(),
        0,
        "the request commits nothing"
    );
}

// ==================== Helpers ====================

/// The bytes of the PATCH that commits before a crash
const FIRST_PATCH: usize = PART + HALF_PART;

/// A session with one PATCH committed, and a second that crashed before its commit.
async fn crash_mid_patch(backend: Backend) -> (Fixture, UploadId, Vec<u8>) {
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(4 * PART + HALF_PART);
    patch(&fixture.storage, &upload_id, &blob[..FIRST_PATCH]).await;

    fixture.faults.crash(Crash::Before, 1, Call::is_commit);
    let error = fixture
        .storage
        .append_upload(&upload_id, Bytes::copy_from_slice(&blob[FIRST_PATCH..]))
        .await
        .unwrap_err();
    assert!(error.to_string().contains(CRASHED), "{error}");
    assert!(
        fixture
            .faults
            .calls()
            .iter()
            .any(|call| matches!(call, Call::PutPart(_, 2))),
        "the lost PATCH uploaded its parts"
    );
    fixture.faults.recover();
    (fixture, upload_id, blob)
}

/// A session whose finalize crashed at the first call that `matches`.
async fn crash_in_finalize<F>(
    backend: Backend,
    crash: Crash,
    matches: F,
) -> (Fixture, UploadId, Vec<u8>)
where
    F: Fn(&Call) -> bool + Send + Sync + 'static,
{
    let fixture = fixture(backend);
    let upload_id = start(&fixture.storage).await;
    let blob = blob_bytes(2 * PART + HALF_PART);
    patch(&fixture.storage, &upload_id, &blob).await;

    fixture.faults.crash(crash, 1, matches);
    fixture
        .storage
        .complete_upload(&upload_id, &Digest::from_sha256_bytes(&blob))
        .await
        .unwrap_err();
    fixture.faults.recover();
    (fixture, upload_id, blob)
}

/// Finishes the session, and checks the blob and that the session is gone.
async fn finish_and_check(fixture: &Fixture, upload_id: &UploadId, blob: &[u8]) {
    let digest = Digest::from_sha256_bytes(blob);
    let stored = fixture
        .storage
        .complete_upload(upload_id, &digest)
        .await
        .unwrap();
    assert_eq!(stored, digest);
    assert_blob(fixture, blob).await;
    assert_session_gone(fixture, upload_id).await;
}

async fn assert_blob(fixture: &Fixture, blob: &[u8]) {
    let stored = fixture
        .raw
        .get(&fixture.blob_key(blob))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(stored == blob, "the digest key holds the blob's bytes");
}

async fn assert_no_blob(fixture: &Fixture, blob: &[u8]) {
    assert!(
        !fixture.raw_exists(&fixture.blob_key(blob)).await,
        "nothing at the digest key"
    );
}

async fn assert_session_gone(fixture: &Fixture, upload_id: &UploadId) {
    assert_eq!(
        fixture
            .raw_objects(&fixture.storage.upload_prefix(upload_id))
            .await,
        Vec::<String>::new(),
        "the session is gone"
    );
}

/// Every object of the session is gone, and each saved upload was aborted.
async fn assert_swept(fixture: &Fixture, upload_id: &UploadId, uploads: &[MultipartId]) {
    assert_session_gone(fixture, upload_id).await;
    let staging = fixture.storage.upload_data_key(upload_id).to_string();
    for upload in uploads {
        assert!(
            fixture
                .faults
                .calls()
                .contains(&Call::Abort(staging.clone(), upload.clone())),
            "{upload} was aborted"
        );
    }
}

/// The staging upload the session's newest state saved.
async fn staging_upload(fixture: &Fixture, upload_id: &UploadId) -> MultipartId {
    fixture
        .storage
        .uploads()
        .load(upload_id)
        .await
        .unwrap()
        .state()
        .upload
        .clone()
        .unwrap()
}

/// Runs the sweep as if every session were long past the upload timeout.
async fn sweep_stale(fixture: &Fixture) {
    let mut sweeper = fixture.storage.sweeper();
    sweeper.clock = Clock::Custom(Arc::new(|| {
        DateTime::try_from(DateTime::TEST.timestamp() + 2 * 3600).unwrap()
    }));
    sweeper.sweep().await;
}

async fn start(storage: &OciStorage) -> UploadId {
    storage.start_upload(&project()).await.unwrap()
}

/// A PATCH whose body arrives in frames of 5 bytes, so frames straddle parts.
async fn patch(storage: &OciStorage, upload_id: &UploadId, bytes: &[u8]) -> Appended {
    patch_in_frames(storage, upload_id, bytes, 5).await
}

async fn patch_in_frames(
    storage: &OciStorage,
    upload_id: &UploadId,
    bytes: &[u8],
    frame: usize,
) -> Appended {
    let session = storage.uploads().load(upload_id).await.unwrap();
    append_in_frames(storage, session, bytes, frame).await
}

/// A PATCH from a session state loaded before it, as a slow request holds one.
async fn append(storage: &OciStorage, session: UploadSession, bytes: &[u8]) -> Appended {
    append_in_frames(storage, session, bytes, 5).await
}

async fn append_in_frames(
    storage: &OciStorage,
    session: UploadSession,
    bytes: &[u8],
    frame: usize,
) -> Appended {
    let frames: Vec<Result<Bytes, Infallible>> = bytes
        .chunks(frame)
        .map(|frame| Ok(Bytes::copy_from_slice(frame)))
        .collect();
    match storage
        .append_upload_body(session, stream::iter(frames))
        .await
    {
        Ok(appended) => appended,
        Err(UploadWriteError::Storage(error)) => panic!("{error}"),
        Err(UploadWriteError::Body(never)) => match never {},
    }
}

/// `len` bytes in a cycle of a prime length, so no part boundary lines up with it.
fn blob_bytes(len: usize) -> Vec<u8> {
    (0..251).cycle().take(len).collect()
}

fn project() -> ProjectUuid {
    PROJECT.parse().unwrap()
}

fn log() -> Logger {
    Logger::root(slog::Discard, slog::o!())
}

#[derive(Debug, Clone, Copy)]
enum Backend {
    Memory,
    File,
}

struct Fixture {
    storage: OciStorage,
    faults: Arc<Faults>,
    /// The store under the wrapper, which never fails
    raw: Arc<dyn ObjectStore>,
    _dir: Option<tempfile::TempDir>,
}

impl Fixture {
    fn blob_key(&self, blob: &[u8]) -> Path {
        self.storage
            .blob_key(&project(), &Digest::from_sha256_bytes(blob))
    }

    async fn raw_exists(&self, key: &Path) -> bool {
        self.raw.head(key).await.is_ok()
    }

    async fn raw_objects(&self, prefix: &Path) -> Vec<String> {
        self.raw
            .list(Some(prefix))
            .map(|meta| meta.unwrap().location.to_string())
            .collect()
            .await
    }
}

fn fixture(backend: Backend) -> Fixture {
    let (raw, multipart, dir): (Arc<dyn ObjectStore>, Arc<dyn MultipartStore>, _) = match backend {
        Backend::Memory => {
            let store = Arc::new(InMemory::new());
            (store.clone(), store, None)
        },
        Backend::File => {
            let dir = tempfile::tempdir().unwrap();
            let store = file_store(dir.path()).unwrap();
            let multipart = Arc::new(FileMultipart::new(Arc::clone(&store)));
            (store, multipart, Some(dir))
        },
    };
    let faults = Arc::new(Faults::default());
    let wrapper = Arc::new(Faulty {
        store: Arc::clone(&raw),
        multipart,
        faults: Arc::clone(&faults),
    });
    let max_body_size = PART as u64 * MAX_PARTS;
    assert!(max_body_size <= DEFAULT_MAX_BODY_SIZE);
    let storage = OciStorage {
        store: Arc::new(RecordingStore::new(wrapper.clone())),
        multipart: Arc::new(RecordingMultipart::new(wrapper)),
        part_copy: None,
        prefix: None,
        upload_timeout: 3600,
        max_body_size,
        chunk_size: PART,
        // One part's worth, so a second request waits while one part uploads
        buffers: BufferPool::new(0, PART as u64),
        log: log(),
        concurrency: 4,
        sweep: Mutex::new(Sweep {
            last: DateTime::TEST.timestamp(),
            ..Sweep::default()
        }),
        clock: Clock::Custom(Arc::new(|| DateTime::TEST)),
    };
    Fixture {
        storage,
        faults,
        raw,
        _dir: dir,
    }
}

// ==================== The wrapper store ====================

const CRASHED: &str = "the store crashed";

/// One call to the wrapper store, by kind and key.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Put(String),
    Get(String),
    Head(String),
    Delete(String),
    List(String),
    Copy(String, String),
    Rename(String, String),
    Create(String),
    PutPart(String, usize),
    Complete(String),
    Abort(String, MultipartId),
}

impl Call {
    /// A put of a session state, which commits it.
    fn is_commit(&self) -> bool {
        if let Self::Put(key) = self {
            key.contains("/states/")
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Crash {
    /// The call fails unsent.
    Before,
    /// The call is sent, and its answer is lost.
    After,
    /// The call alone fails unsent, and the store goes on.
    Once,
    /// The call is sent and its answer alone is lost, and the store goes on.
    AnswerLost,
}

/// What the wrapper does with a call.
enum Decision {
    Forward,
    Fail,
    ForwardThenFail,
}

/// The wrapper's log, its crash trigger, and the gates that tests hold calls at.
#[derive(Default)]
struct Faults {
    calls: Mutex<Vec<Call>>,
    trigger: Mutex<Option<Trigger>>,
    crashed: AtomicBool,
    parts: Mutex<Option<Arc<Semaphore>>>,
    tails: Mutex<Option<Arc<Semaphore>>>,
    commits: Mutex<Option<Arc<Barrier>>>,
}

struct Trigger {
    crash: Crash,
    nth: usize,
    seen: usize,
    matches: Box<dyn Fn(&Call) -> bool + Send + Sync>,
}

impl Faults {
    /// Crashes at the `nth` call that `matches`, and fails every call after it.
    fn crash<F>(&self, crash: Crash, nth: usize, matches: F)
    where
        F: Fn(&Call) -> bool + Send + Sync + 'static,
    {
        *self.trigger.lock().unwrap() = Some(Trigger {
            crash,
            nth,
            seen: 0,
            matches: Box::new(matches),
        });
    }

    /// Ends the crash, as a restarted server would.
    fn recover(&self) {
        *self.trigger.lock().unwrap() = None;
        self.crashed.store(false, Ordering::SeqCst);
    }

    /// Holds each part upload until the test adds a permit to the gate it returns.
    fn hold_parts(&self) -> Arc<Semaphore> {
        let gate = Arc::new(Semaphore::new(0));
        *self.parts.lock().unwrap() = Some(Arc::clone(&gate));
        gate
    }

    /// Holds each read of a committed tail until the test adds a permit to the gate it returns.
    fn hold_tail_reads(&self) -> Arc<Semaphore> {
        let gate = Arc::new(Semaphore::new(0));
        *self.tails.lock().unwrap() = Some(Arc::clone(&gate));
        gate
    }

    /// Holds each commit until `requests` commits wait together, or a while passes.
    fn hold_commits(&self, requests: usize) {
        *self.commits.lock().unwrap() = Some(Arc::new(Barrier::new(requests)));
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    fn clear_calls(&self) {
        self.calls.lock().unwrap().clear();
    }

    fn decide(&self, call: Call) -> Decision {
        let mut trigger = self.trigger.lock().unwrap();
        let decision = if self.crashed.load(Ordering::SeqCst) {
            Decision::Fail
        } else if let Some(trigger) = trigger.as_mut()
            && (trigger.matches)(&call)
        {
            trigger.seen += 1;
            if trigger.seen == trigger.nth {
                if matches!(trigger.crash, Crash::Before | Crash::After) {
                    self.crashed.store(true, Ordering::SeqCst);
                }
                match trigger.crash {
                    Crash::Before | Crash::Once => Decision::Fail,
                    Crash::After | Crash::AnswerLost => Decision::ForwardThenFail,
                }
            } else {
                Decision::Forward
            }
        } else {
            Decision::Forward
        };
        self.calls.lock().unwrap().push(call);
        decision
    }

    async fn call<T, F>(&self, call: Call, forward: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match self.decide(call) {
            Decision::Forward => forward.await,
            Decision::Fail => Err(crashed()),
            Decision::ForwardThenFail => {
                let _lost = forward.await;
                Err(crashed())
            },
        }
    }

    async fn wait_at_commit(&self) {
        let barrier = self.commits.lock().unwrap().clone();
        if let Some(barrier) = barrier {
            let _waited = tokio::time::timeout(Duration::from_millis(500), barrier.wait()).await;
        }
    }
}

impl fmt::Debug for Faults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Faults")
    }
}

fn crashed() -> object_store::Error {
    object_store::Error::Generic {
        store: "Faulty",
        source: CRASHED.into(),
    }
}

/// R2's answer to a completion of an upload it no longer knows, in the shape
/// that `object_store` gives a 404 on a completion.
fn no_such_upload(path: &Path) -> object_store::Error {
    object_store::Error::NotFound {
        path: path.to_string(),
        source: "Server returned non-2xx status code: 404 Not Found: <Error><Code>NoSuchUpload</Code><Message>The specified multipart upload does not exist.</Message></Error>".into(),
    }
}

/// An object store and a multipart store over the same objects, which crash on cue.
struct Faulty {
    store: Arc<dyn ObjectStore>,
    multipart: Arc<dyn MultipartStore>,
    faults: Arc<Faults>,
}

impl fmt::Debug for Faulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Faulty({})", self.store)
    }
}

impl fmt::Display for Faulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Faulty({})", self.store)
    }
}

#[async_trait]
impl ObjectStore for Faulty {
    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> Result<PutResult> {
        let call = Call::Put(location.to_string());
        if call.is_commit() && matches!(opts.mode, PutMode::Create) {
            self.faults.wait_at_commit().await;
        }
        self.faults
            .call(call, self.store.put_opts(location, payload, opts))
            .await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> Result<Box<dyn MultipartUpload>> {
        self.store.put_multipart_opts(location, opts).await
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
        let call = if options.head {
            Call::Head(location.to_string())
        } else {
            Call::Get(location.to_string())
        };
        if let Call::Get(key) = &call
            && key.contains("/tail/")
        {
            let gate = self.faults.tails.lock().unwrap().clone();
            if let Some(gate) = gate {
                gate.acquire().await.unwrap().forget();
            }
        }
        self.faults
            .call(call, self.store.get_opts(location, options))
            .await
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, Result<Path>>,
    ) -> BoxStream<'static, Result<Path>> {
        let faults = Arc::clone(&self.faults);
        let decided = locations
            .map(move |location| {
                let location = location?;
                match faults.decide(Call::Delete(location.to_string())) {
                    Decision::Forward => Ok(location),
                    Decision::Fail | Decision::ForwardThenFail => Err(crashed()),
                }
            })
            .boxed();
        self.store.delete_stream(decided)
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>> {
        let call = Call::List(prefix.map(ToString::to_string).unwrap_or_default());
        match self.faults.decide(call) {
            Decision::Forward => self.store.list(prefix),
            Decision::Fail | Decision::ForwardThenFail => {
                stream::once(async { Err(crashed()) }).boxed()
            },
        }
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        let call = Call::List(prefix.map(ToString::to_string).unwrap_or_default());
        self.faults
            .call(call, self.store.list_with_delimiter(prefix))
            .await
    }

    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        let call = Call::Copy(from.to_string(), to.to_string());
        self.faults
            .call(call, self.store.copy_opts(from, to, options))
            .await
    }

    async fn rename_opts(&self, from: &Path, to: &Path, options: RenameOptions) -> Result<()> {
        let call = Call::Rename(from.to_string(), to.to_string());
        self.faults
            .call(call, self.store.rename_opts(from, to, options))
            .await
    }
}

#[async_trait]
impl MultipartStore for Faulty {
    async fn create_multipart(&self, path: &Path) -> Result<MultipartId> {
        self.faults
            .call(
                Call::Create(path.to_string()),
                self.multipart.create_multipart(path),
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
        let gate = self.faults.parts.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.acquire().await.unwrap().forget();
        }
        self.faults
            .call(
                Call::PutPart(path.to_string(), part_idx),
                self.multipart.put_part(path, id, part_idx, data),
            )
            .await
    }

    /// The in-memory store answers a finished completion with a generic error,
    /// which this answers as R2 does.
    async fn complete_multipart(
        &self,
        path: &Path,
        id: &MultipartId,
        parts: Vec<PartId>,
    ) -> Result<PutResult> {
        self.faults
            .call(
                Call::Complete(path.to_string()),
                self.multipart.complete_multipart(path, id, parts),
            )
            .await
            .map_err(|error| {
                if error.to_string().contains("MultipartUpload not found") {
                    no_such_upload(path)
                } else {
                    error
                }
            })
    }

    async fn abort_multipart(&self, path: &Path, id: &MultipartId) -> Result<()> {
        self.faults
            .call(
                Call::Abort(path.to_string(), id.clone()),
                self.multipart.abort_multipart(path, id),
            )
            .await
    }
}

// ==================== S3 ====================

/// Uploads against a stand-in S3 bucket that keeps what it is sent, as R2 does.
mod s3 {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use bencher_json::DateTime;
    use bencher_json::system::config::{MIN_CHUNK_SIZE, RegistryStorage};
    use tokio::net::TcpListener;
    use tokio::task::JoinSet;

    use super::{
        Clock, Digest, OciStorage, OciStorageError, PROJECT, UploadId, UploadWriteError,
        append_in_frames, blob_bytes, log, patch_in_frames, project, start,
    };
    use crate::mock_s3::{self, Bucket, Request, Response};

    const PART: usize = 5 * 1024 * 1024;
    const FRAME: usize = 64 * 1024;
    /// An `ETag` that is no part's MD5
    const WRONG_E_TAG: &str = "\"00000000000000000000000000000000\"";

    struct S3 {
        storage: OciStorage,
        bucket: Bucket,
        log: mock_s3::Log,
        _tasks: JoinSet<()>,
        _dir: tempfile::TempDir,
    }

    impl S3 {
        fn blob_key(&self, blob: &[u8]) -> String {
            self.storage
                .blob_key(&project(), &Digest::from_sha256_bytes(blob))
                .to_string()
        }

        fn requests(&self, kind: fn(&Request) -> bool) -> Vec<Request> {
            self.log.requests().into_iter().filter(kind).collect()
        }

        async fn complete(
            &self,
            upload_id: &UploadId,
            blob: &[u8],
        ) -> Result<Digest, OciStorageError> {
            self.storage
                .complete_upload(upload_id, &Digest::from_sha256_bytes(blob))
                .await
        }

        fn assert_blob(&self, blob: &[u8]) {
            assert!(
                self.bucket.object(&self.blob_key(blob)).as_deref() == Some(blob),
                "the digest key holds the blob's bytes"
            );
        }

        fn assert_clean(&self) {
            assert_eq!(
                self.bucket.keys("_uploads/"),
                Vec::<String>::new(),
                "no session is left"
            );
            assert_eq!(
                self.bucket.open_uploads(),
                Vec::<String>::new(),
                "no upload is left open"
            );
        }
    }

    /// S3 storage over a stand-in bucket, whose answer `hook` may replace.
    async fn s3<H>(hook: H) -> S3
    where
        H: Fn(&Bucket, &Request) -> Option<Response> + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let bucket = Bucket::default();
        let requests = mock_s3::Log::default();
        let served = bucket.clone();
        let hook = Arc::new(hook);
        let mut tasks = JoinSet::new();
        tasks.spawn(mock_s3::serve(listener, requests.clone(), move |request| {
            let response = hook(&served, &request).unwrap_or_else(|| served.answer(&request));
            async move { response }
        }));
        let dir = tempfile::tempdir().unwrap();
        let storage = OciStorage::try_from_config_with_upload_memory(
            log(),
            Some(RegistryStorage::S3 {
                bucket: mock_s3::BUCKET.to_owned(),
                path: None,
                endpoint: Some(endpoint),
                region: None,
                access_key_id: mock_s3::ACCESS_KEY_ID.to_owned(),
                secret_access_key: mock_s3::SECRET_ACCESS_KEY.parse().unwrap(),
                chunk_size: Some(MIN_CHUNK_SIZE),
            }),
            &dir.path().join("bencher.db"),
            None,
            None,
            None,
            Some(Clock::Custom(Arc::new(|| DateTime::TEST))),
        )
        .unwrap();
        S3 {
            storage,
            bucket,
            log: requests,
            _tasks: tasks,
            _dir: dir,
        }
    }

    /// An upload with every byte appended, ready for its finalize.
    async fn appended(s3: &S3, blob: &[u8]) -> UploadId {
        let upload_id = start(&s3.storage).await;
        let appended = patch_in_frames(&s3.storage, &upload_id, blob, FRAME).await;
        assert!(appended.committed);
        upload_id
    }

    fn is_part_upload(request: &Request) -> bool {
        request.method == "PUT" && request.part_number().is_some() && !request.is_part_copy()
    }

    fn is_staging(request: &Request) -> bool {
        request.key().ends_with("/data")
    }

    fn is_bulk_delete(request: &Request) -> bool {
        request.method == "POST" && request.query("delete").is_some()
    }

    /// The finalize stopped before the promote: no destination opened, nothing copied.
    fn assert_no_promote(s3: &S3) {
        assert!(
            s3.requests(Request::is_part_copy).is_empty(),
            "the finalize stops before the promote"
        );
        assert!(
            !s3.requests(Request::is_create_multipart)
                .iter()
                .any(|request| request.key().contains("/blobs/")),
            "no destination upload opens"
        );
    }

    fn with_e_tag(mut response: Response, e_tag: &str) -> Response {
        response.headers.retain(|(name, _value)| name != "etag");
        response.header("etag", e_tag)
    }

    /// Every answer after `crashed` is set refuses, as a store would that the
    /// server can no longer reach, without a retry.
    fn refused(crashed: &AtomicBool) -> Option<Response> {
        crashed
            .load(Ordering::SeqCst)
            .then(|| Response::error(403, "AccessDenied"))
    }

    /// The parts stream into a multipart upload, and the blob reaches its digest
    /// key by presigned part copies.
    #[tokio::test]
    async fn an_s3_upload_streams_into_parts_and_promotes_by_part_copy() {
        let s3 = s3(|_bucket, _request| None).await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap();

        s3.assert_blob(&blob);
        s3.assert_clean();
        assert_eq!(s3.requests(Request::is_part_copy).len(), 3);
        let refused: Vec<(String, String, u16)> = s3
            .log
            .answers()
            .into_iter()
            .filter(|(request, status)| *status >= 400 && request.method != "HEAD")
            .map(|(request, status)| (request.method.clone(), request.key(), status))
            .collect();
        assert_eq!(refused, [], "the store took every request but the probes");
    }

    /// A part whose `ETag` is not its MD5 uploads again, and the upload finishes.
    #[tokio::test]
    async fn an_s3_part_whose_etag_is_not_its_md5_uploads_again() {
        let spoiled = AtomicU32::new(0);
        let s3 = s3(move |bucket, request| {
            (is_part_upload(request) && spoiled.fetch_add(1, Ordering::SeqCst) == 0)
                .then(|| with_e_tag(bucket.answer(request), WRONG_E_TAG))
        })
        .await;
        let blob = blob_bytes(PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap();

        s3.assert_blob(&blob);
        assert_eq!(
            s3.requests(is_part_upload).len(),
            3,
            "the full part twice, and the tail as the last part"
        );
    }

    /// A part whose `ETag` is never its MD5 fails its PATCH after a second try.
    #[tokio::test]
    async fn an_s3_part_whose_etag_is_never_its_md5_fails_the_patch() {
        let s3 = s3(|bucket, request| {
            is_part_upload(request).then(|| with_e_tag(bucket.answer(request), WRONG_E_TAG))
        })
        .await;
        let upload_id = start(&s3.storage).await;

        let error = s3
            .storage
            .append_upload(&upload_id, blob_bytes(PART + 1000).into())
            .await
            .unwrap_err();

        assert!(matches!(error, OciStorageError::EtagMismatch(_)), "{error}");
        assert_eq!(s3.requests(is_part_upload).len(), 2);
        assert_eq!(s3.storage.get_upload_size(&upload_id).await.unwrap(), 0);
    }

    /// A blob under one part is put again when its `ETag` is not its MD5.
    #[tokio::test]
    async fn an_s3_blob_under_one_part_is_put_again_when_its_etag_is_not_its_md5() {
        let blob = blob_bytes(1000);
        let blob_key = format!(
            "{PROJECT}/blobs/sha256/{}",
            Digest::from_sha256_bytes(&blob).hex_hash()
        );
        let spoiled = AtomicU32::new(0);
        let key = blob_key.clone();
        let s3 = s3(move |bucket, request| {
            (request.method == "PUT"
                && request.key() == key
                && spoiled.fetch_add(1, Ordering::SeqCst) == 0)
                .then(|| with_e_tag(bucket.answer(request), WRONG_E_TAG))
        })
        .await;
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap();

        s3.assert_blob(&blob);
        s3.assert_clean();
        let puts = s3.requests(|request| request.method == "PUT");
        assert_eq!(
            puts.iter()
                .filter(|request| request.key() == blob_key)
                .count(),
            2
        );
        assert!(s3.requests(Request::is_create_multipart).is_empty());
    }

    /// The staging completion is answered 404 `NoSuchUpload` although it completed,
    /// as a retry after a lost answer is: the staging object with the predicted
    /// `ETag` lets the finalize go on.
    #[tokio::test]
    async fn an_s3_completion_lost_to_a_retry_goes_on_with_the_predicted_etag() {
        let s3 = s3(|bucket, request| {
            (request.is_complete() && request.key().ends_with("/data")).then(|| {
                bucket.answer(request);
                Response::error(404, "NoSuchUpload")
            })
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap();

        s3.assert_blob(&blob);
        s3.assert_clean();
    }

    /// The same, with a staging object whose `ETag` is not the predicted one: the
    /// finalize fails, and nothing reaches the digest key.
    #[tokio::test]
    async fn an_s3_completion_lost_to_a_retry_fails_on_another_etag() {
        let s3 = s3(|bucket, request| {
            if !request.key().ends_with("/data") {
                return None;
            }
            if request.is_complete() {
                bucket.answer(request);
                return Some(Response::error(404, "NoSuchUpload"));
            }
            (request.method == "HEAD").then(|| {
                with_e_tag(
                    bucket.answer(request),
                    "\"0123456789abcdef0123456789abcdef-3\"",
                )
            })
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        let error = s3.complete(&upload_id, &blob).await.unwrap_err();

        assert!(matches!(error, OciStorageError::EtagMismatch(_)), "{error}");
        assert!(
            s3.requests(Request::is_part_copy).is_empty(),
            "the finalize stops before the promote"
        );
        assert!(s3.bucket.object(&s3.blob_key(&blob)).is_none());
    }

    /// A finalize that crashed once the destination completed finds the blob on
    /// retry, and only cleans up.
    #[tokio::test]
    async fn a_resumed_promote_whose_blob_is_promoted_copies_nothing() {
        let crashed = Arc::new(AtomicBool::new(false));
        let crash = Arc::clone(&crashed);
        let armed = AtomicBool::new(true);
        let s3 = s3(move |bucket, request| {
            if let Some(refusal) = refused(&crash) {
                return Some(refusal);
            }
            (request.is_complete()
                && request.key().contains("/blobs/")
                && armed.swap(false, Ordering::SeqCst))
            .then(|| {
                bucket.answer(request);
                crash.store(true, Ordering::SeqCst);
                Response::error(403, "AccessDenied")
            })
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;
        s3.complete(&upload_id, &blob).await.unwrap_err();
        s3.assert_blob(&blob);
        crashed.store(false, Ordering::SeqCst);
        let copies = s3.requests(Request::is_part_copy).len();

        s3.complete(&upload_id, &blob).await.unwrap();

        assert_eq!(
            s3.requests(Request::is_part_copy).len(),
            copies,
            "no copy again"
        );
        s3.assert_blob(&blob);
        s3.assert_clean();
    }

    /// A destination upload that is gone, as the bucket's own expiry would leave
    /// it, is aborted and replaced, and the copy runs again.
    #[tokio::test]
    async fn a_resumed_promote_whose_destination_is_gone_opens_another() {
        let crashed = Arc::new(AtomicBool::new(false));
        let crash = Arc::clone(&crashed);
        let armed = AtomicBool::new(true);
        let s3 = s3(move |_bucket, request| {
            if request.is_part_copy() && armed.swap(false, Ordering::SeqCst) {
                crash.store(true, Ordering::SeqCst);
            }
            refused(&crash)
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;
        s3.complete(&upload_id, &blob).await.unwrap_err();
        crashed.store(false, Ordering::SeqCst);
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        let super::Phase::Promoting(promotion) = session.state().phase.clone() else {
            panic!("the session is promoting");
        };
        let gone = promotion.destination.unwrap();
        s3.bucket.drop_upload(&gone);

        s3.complete(&upload_id, &blob).await.unwrap();

        s3.assert_blob(&blob);
        s3.assert_clean();
        let aborts = s3.requests(Request::is_abort);
        assert!(
            aborts
                .iter()
                .any(|request| request.query("uploadId").as_deref() == Some(gone.as_str())),
            "the gone upload is aborted"
        );
    }

    /// A destination upload is replaced once per request: when the new one is
    /// gone too, the finalize fails instead of opening uploads without end.
    #[tokio::test]
    async fn a_destination_gone_again_fails_the_finalize() {
        let s3 = s3(|_bucket, request| {
            request
                .is_part_copy()
                .then(|| Response::error(404, "NoSuchUpload"))
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        let finished = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            s3.complete(&upload_id, &blob),
        )
        .await
        .expect("the finalize gives up");

        finished.unwrap_err();
        let opened = s3
            .requests(Request::is_create_multipart)
            .iter()
            .filter(|request| request.key().contains("/blobs/"))
            .count();
        assert_eq!(opened, 2, "one destination, and one replacement");
    }

    /// A finalize that loses the `Promoting` commit to another aborts only the
    /// destination upload it opened, and promotes into the winner's.
    #[tokio::test]
    async fn a_finalize_that_loses_the_promoting_commit_aborts_only_its_own_destination() {
        let other = Arc::new(Mutex::new(None::<String>));
        let winner = Arc::clone(&other);
        let s3 = s3(move |bucket, request| {
            if !(request.is_create_multipart() && request.key().contains("/blobs/")) {
                return None;
            }
            let mut winner = winner.lock().unwrap();
            if winner.is_some() {
                return None;
            }
            let ours = bucket.answer(request);
            *winner = Some(commit_another_promotion(bucket));
            Some(ours)
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap();

        s3.assert_blob(&blob);
        s3.assert_clean();
        let other = other.lock().unwrap().clone().unwrap();
        let aborted: Vec<String> = s3
            .requests(Request::is_abort)
            .iter()
            .filter_map(|request| request.query("uploadId"))
            .collect();
        assert_eq!(aborted.len(), 1, "{aborted:?}");
        assert_ne!(aborted[0], other, "the winner's upload is never aborted");
        assert!(
            s3.requests(Request::is_part_copy)
                .iter()
                .all(|request| request.query("uploadId").as_deref() == Some(other.as_str())),
            "the parts copy into the winner's upload"
        );
    }

    /// A PUT above one part whose digest does not match aborts the staging upload
    /// it opened, and removes the session.
    #[tokio::test]
    async fn an_s3_put_above_one_part_with_another_digest_leaves_no_open_upload() {
        let s3 = s3(|_bucket, _request| None).await;
        let upload_id = start(&s3.storage).await;
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        let frames: Vec<Result<bytes::Bytes, std::convert::Infallible>> =
            blob_bytes(2 * PART + 1000)
                .chunks(FRAME)
                .map(|frame| Ok(bytes::Bytes::copy_from_slice(frame)))
                .collect();

        let error = s3
            .storage
            .complete_upload_body(
                session,
                &Digest::from_sha256_bytes(b"another blob"),
                futures::stream::iter(frames),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                UploadWriteError::Storage(OciStorageError::DigestMismatch { .. })
            ),
            "{error:?}"
        );
        assert!(!s3.requests(Request::is_create_multipart).is_empty());
        s3.assert_clean();
    }

    /// A first PATCH that opens the staging upload and loses the commit aborts the
    /// upload it opened, which no committed state names.
    #[tokio::test]
    async fn an_s3_first_patch_that_loses_its_commit_leaves_no_open_upload() {
        let armed = AtomicBool::new(true);
        let s3 = s3(move |bucket, request| {
            if request.method == "PUT"
                && request.key().ends_with("/states/00000000000000000001")
                && armed.swap(false, Ordering::SeqCst)
            {
                let first = request.key().replace(
                    "/states/00000000000000000001",
                    "/states/00000000000000000000",
                );
                bucket.put(&request.key(), bucket.object(&first).unwrap());
            }
            None
        })
        .await;
        let upload_id = start(&s3.storage).await;

        let appended =
            patch_in_frames(&s3.storage, &upload_id, &blob_bytes(PART + 1000), FRAME).await;

        assert!(!appended.committed);
        assert_eq!(appended.offset, 0, "the other request committed nothing");
        assert!(!s3.requests(Request::is_create_multipart).is_empty());
        assert_eq!(s3.bucket.open_uploads(), Vec::<String>::new());
    }

    /// A PATCH whose commit `object_store` retries into a 412, after the store kept
    /// the first attempt, committed: it keeps its staging upload, and the PUT
    /// that follows finishes.
    #[tokio::test]
    async fn an_s3_patch_whose_retried_commit_finds_its_own_state_keeps_its_upload() {
        let armed = Arc::new(AtomicBool::new(false));
        let hook = Arc::clone(&armed);
        let s3 = s3(move |bucket, request| {
            if hook.load(Ordering::SeqCst) {
                store_then_fail(bucket, request)
            } else {
                None
            }
        })
        .await;
        let upload_id = start(&s3.storage).await;
        armed.store(true, Ordering::SeqCst);
        let blob = blob_bytes(PART + 1000);
        let (first, rest) = blob.split_at(PART);

        let appended = patch_in_frames(&s3.storage, &upload_id, first, FRAME).await;

        assert!(appended.committed, "the state there is this request's own");
        assert_eq!(appended.offset, PART as u64);
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        s3.storage
            .complete_upload_body(
                session,
                &Digest::from_sha256_bytes(&blob),
                futures::stream::iter(body_of(rest)),
            )
            .await
            .unwrap();
        s3.assert_blob(&blob);
        s3.assert_clean();
    }

    /// The Docker shape, a POST and then 1 PUT with the whole body, finishes when
    /// `object_store` retries every state put into a 412 after the store kept it.
    #[tokio::test]
    async fn an_s3_post_and_put_whose_state_puts_are_all_retried_finish() {
        let s3 = s3(store_then_fail).await;
        let blob = blob_bytes(2 * PART + 1000);

        let upload_id = start(&s3.storage).await;
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        s3.storage
            .complete_upload_body(
                session,
                &Digest::from_sha256_bytes(&blob),
                futures::stream::iter(body_of(&blob)),
            )
            .await
            .unwrap();

        s3.assert_blob(&blob);
        s3.assert_clean();
    }

    /// A commit retried into a 412 whose state cannot be read back may be this
    /// request's: the PATCH fails, and its staging upload stays for the sweep.
    #[tokio::test]
    async fn an_s3_patch_that_cannot_read_back_its_retried_commit_keeps_its_upload() {
        let armed = Arc::new(AtomicBool::new(false));
        let hook = Arc::clone(&armed);
        let s3 = s3(move |bucket, request| {
            if hook.load(Ordering::SeqCst) {
                if request.method == "GET" && request.key().contains("/states/") {
                    Some(Response::error(403, "AccessDenied"))
                } else {
                    store_then_fail(bucket, request)
                }
            } else {
                None
            }
        })
        .await;
        let upload_id = start(&s3.storage).await;
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        armed.store(true, Ordering::SeqCst);

        let error = s3
            .storage
            .append_upload_body(
                session,
                futures::stream::iter(body_of(&blob_bytes(PART + 1000))),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, UploadWriteError::Storage(_)), "{error:?}");
        assert_eq!(
            s3.bucket.open_uploads().len(),
            1,
            "the staging upload stays for the sweep"
        );
    }

    /// A PUT that opened its staging upload and then stalls past the fill deadline
    /// aborts the upload it opened, which no committed state names.
    #[tokio::test]
    async fn an_s3_put_that_stalls_aborts_the_upload_it_opened() {
        use futures::StreamExt as _;

        let s3 = s3(|_bucket, _request| None).await;
        let upload_id = start(&s3.storage).await;
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        let blob = blob_bytes(PART + 1000);
        let stalling = futures::stream::iter(body_of(&blob)).chain(futures::stream::pending());

        // Paused time jumps to the next timer whenever the runtime waits, so the
        // ticker keeps each jump short while the mock store answers.
        tokio::time::pause();
        let answered = AtomicBool::new(false);
        let stalled = async {
            let failed = s3
                .storage
                .complete_upload_body(session, &Digest::from_sha256_bytes(&blob), stalling)
                .await;
            answered.store(true, Ordering::SeqCst);
            failed
        };
        let ticks = async {
            while !answered.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        };
        let (failed, ()) = tokio::time::timeout(std::time::Duration::from_hours(1), async {
            tokio::join!(stalled, ticks)
        })
        .await
        .expect("the stalled PUT fails");
        tokio::time::resume();

        let Err(UploadWriteError::Storage(OciStorageError::PartStalled(_))) = failed else {
            panic!("{failed:?}");
        };
        assert!(
            !s3.requests(Request::is_create_multipart).is_empty(),
            "the PUT opened its staging upload"
        );
        assert_eq!(s3.bucket.open_uploads(), Vec::<String>::new());
    }

    /// Stores the first put of each state generation and answers it 500, so that
    /// `object_store` retries the put and finds the key taken: a 412.
    fn store_then_fail(bucket: &Bucket, request: &Request) -> Option<Response> {
        let key = request.key();
        if request.method != "PUT" || !key.contains("/states/") || bucket.object(&key).is_some() {
            return None;
        }
        bucket.put(&key, request.body.clone());
        Some(Response::error(500, "InternalError"))
    }

    fn body_of(bytes: &[u8]) -> Vec<Result<bytes::Bytes, std::convert::Infallible>> {
        bytes
            .chunks(FRAME)
            .map(|frame| Ok(bytes::Bytes::copy_from_slice(frame)))
            .collect()
    }

    /// A PATCH whose body fails after it opened the staging upload aborts it.
    #[tokio::test]
    async fn an_s3_patch_whose_body_fails_leaves_no_open_upload() {
        let s3 = s3(|_bucket, _request| None).await;
        let upload_id = start(&s3.storage).await;
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        let mut frames: Vec<Result<bytes::Bytes, &str>> = blob_bytes(PART + 1000)
            .chunks(FRAME)
            .map(|frame| Ok(bytes::Bytes::copy_from_slice(frame)))
            .collect();
        frames.push(Err("the client went away"));

        let error = s3
            .storage
            .append_upload_body(session, futures::stream::iter(frames))
            .await
            .unwrap_err();

        assert!(matches!(error, UploadWriteError::Body(_)), "{error:?}");
        assert!(!s3.requests(Request::is_create_multipart).is_empty());
        assert_eq!(s3.bucket.open_uploads(), Vec::<String>::new());
        assert_eq!(s3.storage.get_upload_size(&upload_id).await.unwrap(), 0);
    }

    /// Copies that do not read the bytes the staging upload completed with give
    /// a promoted blob whose `ETag` is not the staging object's: the finalize
    /// fails, and the digest key is deleted.
    #[tokio::test]
    async fn an_s3_promote_whose_copies_do_not_match_the_staging_object_fails() {
        let armed = AtomicBool::new(true);
        let s3 = s3(move |bucket, request| {
            if request.is_part_copy() && armed.swap(false, Ordering::SeqCst) {
                let staging = bucket
                    .keys("_uploads/")
                    .into_iter()
                    .find(|key| key.ends_with("/data"))
                    .unwrap();
                let changed: Vec<u8> = bucket
                    .object(&staging)
                    .unwrap()
                    .iter()
                    .map(|byte| byte ^ 0xff)
                    .collect();
                bucket.put(&staging, changed);
            }
            None
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        let error = s3.complete(&upload_id, &blob).await.unwrap_err();

        assert!(matches!(error, OciStorageError::EtagMismatch(_)), "{error}");
        assert!(!s3.requests(Request::is_part_copy).is_empty());
        assert!(
            s3.bucket.object(&s3.blob_key(&blob)).is_none(),
            "the mismatched blob is deleted"
        );
    }

    /// A blob under one part whose `ETag` is never its MD5 fails, and is deleted.
    #[tokio::test]
    async fn an_s3_blob_under_one_part_whose_etag_is_never_its_md5_is_deleted() {
        let blob = blob_bytes(1000);
        let blob_key = format!(
            "{PROJECT}/blobs/sha256/{}",
            Digest::from_sha256_bytes(&blob).hex_hash()
        );
        let s3 = s3(move |bucket, request| {
            (request.method == "PUT" && request.key() == blob_key)
                .then(|| with_e_tag(bucket.answer(request), WRONG_E_TAG))
        })
        .await;
        let upload_id = appended(&s3, &blob).await;

        let error = s3.complete(&upload_id, &blob).await.unwrap_err();

        assert!(matches!(error, OciStorageError::EtagMismatch(_)), "{error}");
        assert!(s3.bucket.object(&s3.blob_key(&blob)).is_none());
    }

    /// A staging object whose `ETag` is not the multipart `ETag` of its parts fails
    /// the finalize before the promote.
    #[tokio::test]
    async fn an_s3_staging_object_with_another_etag_fails_before_the_promote() {
        let s3 = s3(|bucket, request| {
            (request.method == "HEAD" && is_staging(request)).then(|| {
                with_e_tag(
                    bucket.answer(request),
                    "\"0123456789abcdef0123456789abcdef-3\"",
                )
            })
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        let error = s3.complete(&upload_id, &blob).await.unwrap_err();

        assert!(matches!(error, OciStorageError::EtagMismatch(_)), "{error}");
        assert_no_promote(&s3);
        assert!(s3.bucket.object(&s3.blob_key(&blob)).is_none());
    }

    /// A staging object of another size than the upload's fails the finalize
    /// before the promote, since every copy range comes from its size.
    #[tokio::test]
    async fn an_s3_staging_object_of_another_size_fails_before_the_promote() {
        let s3 = s3(|bucket, request| {
            (request.method == "HEAD" && is_staging(request)).then(|| {
                let mut response = bucket.answer(request);
                for (name, value) in &mut response.headers {
                    if name == "content-length" {
                        *value = (value.parse::<u64>().unwrap() - 1).to_string();
                    }
                }
                response
            })
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap_err();

        assert_no_promote(&s3);
        assert!(s3.bucket.object(&s3.blob_key(&blob)).is_none());
    }

    /// Crash table row 4 on S3: a finalize lost mid-copy copies again into the
    /// destination upload its state saved.
    #[tokio::test]
    async fn a_promote_lost_mid_copy_copies_again_into_the_saved_destination() {
        let crashed = Arc::new(AtomicBool::new(false));
        let crash = Arc::clone(&crashed);
        let armed = AtomicBool::new(true);
        let s3 = s3(move |_bucket, request| {
            if request.is_part_copy() && armed.swap(false, Ordering::SeqCst) {
                crash.store(true, Ordering::SeqCst);
            }
            refused(&crash)
        })
        .await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;
        s3.complete(&upload_id, &blob).await.unwrap_err();
        crashed.store(false, Ordering::SeqCst);
        let session = s3.storage.uploads().load(&upload_id).await.unwrap();
        let super::Phase::Promoting(promotion) = session.state().phase.clone() else {
            panic!("the session is promoting");
        };
        let saved = promotion.destination.unwrap();
        let opened = s3.requests(Request::is_create_multipart).len();

        s3.complete(&upload_id, &blob).await.unwrap();

        assert_eq!(
            s3.requests(Request::is_create_multipart).len(),
            opened,
            "no destination opens again"
        );
        let copies: Vec<String> = s3
            .requests(Request::is_part_copy)
            .iter()
            .filter_map(|request| request.query("uploadId"))
            .collect();
        assert!(
            copies.iter().all(|copy| *copy == saved),
            "every copy goes into the saved destination"
        );
        s3.assert_blob(&blob);
        s3.assert_clean();
    }

    /// Two PATCH requests from one state on S3, the loser after the winner commits:
    /// the loser's part replaces the winner's, so R2 refuses the winner's part at
    /// the completion; the client starts the blob again, and nothing is stored.
    #[tokio::test]
    async fn racing_s3_patches_never_store_wrong_bytes() {
        let s3 = s3(|_bucket, _request| None).await;
        let upload_id = start(&s3.storage).await;
        let first = blob_bytes(PART);
        patch_in_frames(&s3.storage, &upload_id, &first, FRAME).await;
        let winner = blob_bytes(2 * PART)[PART..].to_vec();
        let loser: Vec<u8> = winner.iter().map(|byte| byte ^ 0xff).collect();
        let winner_session = s3.storage.uploads().load(&upload_id).await.unwrap();
        let loser_session = s3.storage.uploads().load(&upload_id).await.unwrap();

        let won = append_in_frames(&s3.storage, winner_session, &winner, FRAME).await;
        let lost = append_in_frames(&s3.storage, loser_session, &loser, FRAME).await;

        assert!(won.committed && !lost.committed);
        let winning_blob = [first.as_slice(), &winner].concat();
        let error = s3.complete(&upload_id, &winning_blob).await.unwrap_err();
        assert!(
            matches!(error, OciStorageError::UploadNotFound(_)),
            "{error}"
        );
        assert!(s3.bucket.object(&s3.blob_key(&winning_blob)).is_none());
        let losing_blob = [first.as_slice(), &loser].concat();
        assert!(s3.bucket.object(&s3.blob_key(&losing_blob)).is_none());
    }

    /// The cleanup before the 201 deletes in 2 requests: the staging object and the
    /// tails, then the states.
    #[tokio::test]
    async fn an_s3_finalize_cleans_up_in_two_delete_requests() {
        let s3 = s3(|_bucket, _request| None).await;
        let blob = blob_bytes(2 * PART + 1000);
        let upload_id = appended(&s3, &blob).await;

        s3.complete(&upload_id, &blob).await.unwrap();

        let deletes = s3.requests(is_bulk_delete);
        assert_eq!(deletes.len(), 2, "{deletes:?}");
        let states = String::from_utf8_lossy(&deletes[1].body).into_owned();
        assert!(
            states.contains("/states/") && !states.contains("/data") && !states.contains("/tail/"),
            "{states}"
        );
        s3.assert_clean();
    }

    /// Commits a `Promoting` state over the session's newest, as a request that
    /// wins the commit would, and returns the destination upload it opened.
    fn commit_another_promotion(bucket: &Bucket) -> String {
        let newest = bucket
            .keys("_uploads/")
            .into_iter()
            .filter(|key| key.contains("/states/"))
            .max()
            .unwrap();
        let mut state: serde_json::Value =
            serde_json::from_slice(&bucket.object(&newest).unwrap()).unwrap();
        let finished = state["phase"].clone();
        let staging = format!("{}/data", newest.split("/states/").next().unwrap());
        let destination = bucket.open_upload();
        state["phase"] = serde_json::json!({
            "name": "promoting",
            "finished": {
                "digest": finished["digest"],
                "size": finished["size"],
                "parts": finished["parts"],
            },
            "staging_e_tag": bucket.e_tag(&staging),
            "destination": destination,
        });
        let generation: u64 = newest.rsplit('/').next().unwrap().parse().unwrap();
        let next = format!(
            "{}/states/{:020}",
            newest.split("/states/").next().unwrap(),
            generation + 1
        );
        bucket.put(&next, serde_json::to_vec(&state).unwrap());
        destination
    }
}
