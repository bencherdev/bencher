#![cfg(feature = "plus")]
#![expect(unused_crate_dependencies, reason = "integration test file")]
//! Integration tests for in-process replication (`bencher_replica`) against
//! a full API server: real Dropshot server, real writes through the API,
//! step-driven replication engine, restore-and-compare verification.

#[cfg(test)]
mod cases {
    use bencher_api_tests::TestServer;
    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use bencher_replica::testing::assert_replica_equivalent;
    use bencher_replica::{
        CheckpointOutcome, EngineState, ReplicaConfig, RestoreOutcome, SyncEngine,
        restore_if_missing,
    };
    use camino::{Utf8Path, Utf8PathBuf};

    fn dir_path(tmp: &tempfile::TempDir) -> Utf8PathBuf {
        Utf8Path::from_path(tmp.path())
            .expect("tempdir path is UTF-8")
            .to_path_buf()
    }

    fn replica_json(root: &Utf8Path) -> JsonReplication {
        JsonReplication {
            target: ReplicationTarget::File {
                path: root.to_path_buf().into_std_path_buf(),
            },
            sync_interval_secs: None,
            checkpoint_interval_secs: None,
            min_checkpoint_pages: None,
            snapshot_interval_secs: None,
            snapshot_throttle_mib: None,
            retention_generations: None,
            verification_interval_secs: None,
            shutdown_sync_timeout_secs: None,
        }
    }

    fn logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    // A fixed, injected clock. The repo rule is that time-based tests inject a
    // clock instead of racing the wall clock, and a frozen clock is also
    // strictly safer here: the SyncEngine's only clock-driven work is the
    // due-ness of periodic checkpoints, verification, and snapshots. With time
    // frozen, none of those become due, so these tests exercise exactly the
    // steps they drive explicitly (the bootstrap snapshot, ship, and
    // checkpoint). The initial-generation snapshot is driven by
    // `pending_new_generation` at construction, not the clock, so freezing time
    // does not gate `until_streaming` into a hang.
    fn test_clock() -> bencher_json::Clock {
        bencher_json::Clock::Custom(std::sync::Arc::new(|| bencher_json::DateTime::TEST))
    }

    /// Drive the engine through the fresh-replica bootstrap snapshot.
    async fn until_streaming<C: Send + 'static>(engine: &mut SyncEngine<C>) {
        for _ in 0u8..64 {
            if engine.state() == EngineState::Streaming {
                return;
            }
            engine.sync_once().await.expect("bootstrap sync");
        }
        panic!(
            "engine never reached Streaming; state: {:?}",
            engine.state()
        );
    }

    // Real API writes replicate, and the replica restores to an equivalent
    // database.
    #[tokio::test]
    async fn server_writes_replicate_and_restore() {
        let replica_tmp = tempfile::tempdir().expect("replica tempdir");
        let replica_root = dir_path(&replica_tmp);
        let (server, mut engine) =
            TestServer::new_with_replica(replica_json(&replica_root), Some(test_clock()), false)
                .await;
        until_streaming(&mut engine).await;

        // Writes through the real API.
        let _admin = server.signup("Replica Admin", "replica@example.com").await;
        let _user = server
            .signup("Replica User", "replicauser@example.com")
            .await;
        engine.sync_once().await.expect("ship API writes");

        // The replica restores to a logically equivalent database.
        let restore_tmp = tempfile::tempdir().expect("restore tempdir");
        let target_db = dir_path(&restore_tmp).join("restored.db");
        let config = ReplicaConfig::try_from(replica_json(&replica_root)).expect("config");
        let outcome = restore_if_missing(&logger(), &config, &target_db)
            .await
            .expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::Restored { .. }),
            "expected Restored, got {outcome:?}"
        );
        assert_replica_equivalent(
            Utf8Path::from_path(server.db_path()).expect("db path is UTF-8"),
            &target_db,
        );
    }

    // The startup handshake: a missing database file is rebuilt from the
    // replica, and the signed-up user is present in the restored database.
    #[tokio::test]
    async fn server_reboot_missing_db_auto_restores() {
        let replica_tmp = tempfile::tempdir().expect("replica tempdir");
        let replica_root = dir_path(&replica_tmp);
        let (server, mut engine) =
            TestServer::new_with_replica(replica_json(&replica_root), Some(test_clock()), false)
                .await;
        until_streaming(&mut engine).await;
        let _admin = server.signup("Reboot Admin", "reboot@example.com").await;
        engine.sync_once().await.expect("ship API writes");
        drop(engine);
        drop(server);

        // "Reboot": the volume is gone; the same restore-if-missing handshake
        // that main.rs runs rebuilds the database before any connection opens.
        let boot_tmp = tempfile::tempdir().expect("boot tempdir");
        let new_db = dir_path(&boot_tmp).join("bencher.db");
        let config = ReplicaConfig::try_from(replica_json(&replica_root)).expect("config");
        let outcome = restore_if_missing(&logger(), &config, &new_db)
            .await
            .expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::Restored { .. }),
            "expected Restored, got {outcome:?}"
        );
        let conn = rusqlite::Connection::open(&new_db).expect("open restored db");
        let users: i64 = conn
            .query_row(
                "SELECT count(*) FROM user WHERE email = 'reboot@example.com'",
                [],
                |row| row.get(0),
            )
            .expect("query restored user");
        assert_eq!(users, 1, "signed-up user must survive the reboot restore");
    }

    // A fresh server over an empty replica is a clean fresh start, and the
    // existing database is never touched.
    #[tokio::test]
    async fn server_boot_empty_replica_fresh_start() {
        let replica_tmp = tempfile::tempdir().expect("replica tempdir");
        let replica_root = dir_path(&replica_tmp);
        let config = ReplicaConfig::try_from(replica_json(&replica_root)).expect("config");

        // Empty replica, missing database: NoReplica (fresh server).
        let boot_tmp = tempfile::tempdir().expect("boot tempdir");
        let new_db = dir_path(&boot_tmp).join("bencher.db");
        let outcome = restore_if_missing(&logger(), &config, &new_db)
            .await
            .expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::NoReplica),
            "expected NoReplica, got {outcome:?}"
        );
        assert!(!new_db.exists(), "fresh start must not create a database");

        // Existing database: Skipped, file untouched.
        let (server, mut engine) =
            TestServer::new_with_replica(replica_json(&replica_root), Some(test_clock()), false)
                .await;
        until_streaming(&mut engine).await;
        let outcome = restore_if_missing(
            &logger(),
            &config,
            Utf8Path::from_path(server.db_path()).expect("db path is UTF-8"),
        )
        .await
        .expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::Skipped),
            "expected Skipped, got {outcome:?}"
        );
    }

    // Shadow mode: the replica ships and restores alongside a (nominal)
    // Litestream, but never checkpoints.
    #[tokio::test]
    async fn server_shadow_mode_ships_without_checkpoints() {
        let replica_tmp = tempfile::tempdir().expect("replica tempdir");
        let replica_root = dir_path(&replica_tmp);
        let (server, mut engine) =
            TestServer::new_with_replica(replica_json(&replica_root), Some(test_clock()), true)
                .await;
        until_streaming(&mut engine).await;
        let _admin = server.signup("Shadow Admin", "shadow@example.com").await;
        engine.sync_once().await.expect("ship API writes");

        // Shadow never checkpoints: Litestream keeps checkpoint ownership.
        let outcome = engine.checkpoint_once().await.expect("checkpoint");
        assert_eq!(outcome, CheckpointOutcome::SkippedShadow);

        // The shadow replica still restores to an equivalent database.
        let restore_tmp = tempfile::tempdir().expect("restore tempdir");
        let target_db = dir_path(&restore_tmp).join("restored.db");
        let config = ReplicaConfig::try_from(replica_json(&replica_root)).expect("config");
        let outcome = restore_if_missing(&logger(), &config, &target_db)
            .await
            .expect("restore");
        assert!(
            matches!(outcome, RestoreOutcome::Restored { .. }),
            "expected Restored, got {outcome:?}"
        );
        assert_replica_equivalent(
            Utf8Path::from_path(server.db_path()).expect("db path is UTF-8"),
            &target_db,
        );
    }
}
