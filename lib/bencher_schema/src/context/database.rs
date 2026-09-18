use std::{path::PathBuf, sync::Arc};

use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use dropshot::HttpError;

use crate::error::issue_error;

pub type DbConnection = diesel::SqliteConnection;

pub struct Database {
    pub path: PathBuf,
    pub busy_timeout: u32,
    /// The public database connection pool.
    /// Unauthenticated requests should only use this pool.
    pub public_pool: Pool<ConnectionManager<DbConnection>>,
    /// The authenticated database connection pool.
    /// Authenticated requests may use this pool.
    pub auth_pool: Pool<ConnectionManager<DbConnection>>,
    /// The database connection for write operations.
    pub connection: Arc<tokio::sync::Mutex<DbConnection>>,
}

impl Database {
    pub async fn get_public_conn(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<DbConnection>>, HttpError> {
        if let Some(conn) = self.public_pool.try_get() {
            Ok(conn)
        } else {
            Self::get_conn(self.public_pool.clone()).await
        }
    }

    pub async fn get_auth_conn(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<DbConnection>>, HttpError> {
        if let Some(conn) = self.public_pool.try_get() {
            Ok(conn)
        } else if let Some(conn) = self.auth_pool.try_get() {
            Ok(conn)
        } else {
            Self::get_conn(self.auth_pool.clone()).await
        }
    }

    async fn get_conn(
        pool: Pool<ConnectionManager<DbConnection>>,
    ) -> Result<PooledConnection<ConnectionManager<DbConnection>>, HttpError> {
        tokio::task::spawn_blocking(move || {
            pool.get().map_err(|e| {
                issue_error(
                    "Failed to get database connection from pool",
                    "Failed to get a database connection from pool:",
                    e,
                )
            })
        })
        .await
        .map_err(|e| {
            issue_error(
                "Failed to join database connection task from pool",
                "Failed to join the database connection task from pool:",
                e,
            )
        })?
    }
}
