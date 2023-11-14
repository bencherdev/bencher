#![cfg(feature = "plus")]

use std::sync::Arc;

use bencher_json::{DateTime, JsonServer, JsonServerStats, ServerUuid, BENCHER_API_URL};
use chrono::{Duration, NaiveTime, Utc};
use diesel::RunQueryDsl;
use dropshot::HttpError;
use slog::Logger;

use crate::{
    context::{Body, DbConnection, Message, Messenger, ServerStatsBody},
    error::resource_conflict_err,
    model::user::QueryUser,
    schema::{self, server as server_table},
    util::fn_get::fn_get,
};

mod stats;

crate::util::typed_id::typed_id!(ServerId);

const SERVER_ID: ServerId = ServerId(1);

#[derive(Debug, Clone, Copy, diesel::Queryable)]
pub struct QueryServer {
    pub id: ServerId,
    pub uuid: ServerUuid,
    pub created: DateTime,
}

impl QueryServer {
    fn_get!(server, ServerId);

    pub fn get_server(conn: &mut DbConnection) -> Result<Self, HttpError> {
        Self::get(conn, SERVER_ID)
    }

    pub fn get_or_create(conn: &mut DbConnection) -> Result<Self, HttpError> {
        if let Ok(server) = Self::get_server(conn) {
            Ok(server)
        } else {
            let server = InsertServer::default();
            diesel::insert_into(schema::server::table)
                .values(&server)
                .execute(conn)
                .map_err(resource_conflict_err!(Server, SERVER_ID))?;
            Self::get_server(conn)
        }
    }

    pub fn spawn_stats(
        self,
        log: Logger,
        conn: Arc<tokio::sync::Mutex<DbConnection>>,
        offset: NaiveTime,
        messenger: Option<Messenger>,
    ) {
        tokio::spawn(async move {
            loop {
                let now = Utc::now().naive_utc().time();
                let sleep_time = match now.cmp(&offset) {
                    std::cmp::Ordering::Less => offset - now,
                    std::cmp::Ordering::Equal => Duration::days(1),
                    std::cmp::Ordering::Greater => Duration::days(1) - (now - offset),
                }
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(24 * 60 * 60));
                tokio::time::sleep(sleep_time).await;

                let conn = &mut *conn.lock().await;
                let json_stats = match self.get_stats(conn, messenger.is_some()) {
                    Ok(json_stats) => json_stats,
                    Err(e) => {
                        slog::error!(log, "Failed to get stats: {e}");
                        continue;
                    },
                };
                let json_stats_str = match serde_json::to_string(&json_stats) {
                    Ok(json_stats_str) => json_stats_str,
                    Err(e) => {
                        slog::error!(log, "Failed to serialize stats: {e}");
                        continue;
                    },
                };

                if let Some(messenger) = messenger.as_ref() {
                    slog::info!(log, "Bencher Cloud Stats: {json_stats_str:?}");
                    if let Err(e) =
                        Self::send_stats_to_backend(&log, conn, messenger, &json_stats_str, true)
                    {
                        slog::error!(log, "Failed to send stats: {e}");
                    }
                } else {
                    let client = reqwest::Client::new();
                    if let Err(e) = client
                        .post(BENCHER_API_URL.clone())
                        .body(json_stats_str)
                        .send()
                        .await
                    {
                        slog::error!(log, "Failed to send stats: {e}");
                    }
                }
            }
        });
    }

    pub fn get_stats(
        self,
        conn: &mut DbConnection,
        is_bencher_cloud: bool,
    ) -> Result<JsonServerStats, HttpError> {
        stats::get_stats(conn, self, is_bencher_cloud)
    }

    pub fn send_stats_to_backend(
        log: &Logger,
        conn: &mut DbConnection,
        messenger: &Messenger,
        server_stats: &str,
        is_bencher_cloud: bool,
    ) -> Result<(), HttpError> {
        // TODO find a better home for these than my inbox
        let admins = QueryUser::get_admins(conn)?;
        for admin in admins {
            let message = Message {
                to_name: Some(admin.name.clone().into()),
                to_email: admin.email.into(),
                subject: Some(format!(
                    "🐰 {host} Server Stats",
                    host = if is_bencher_cloud {
                        "Bencher Cloud"
                    } else {
                        "Self-Hosted"
                    }
                )),
                body: Some(Body::ServerStats(ServerStatsBody {
                    server_stats: server_stats.to_owned(),
                })),
            };
            messenger.send(log, message);
        }
        Ok(())
    }

    pub fn into_json(self) -> JsonServer {
        let Self { uuid, created, .. } = self;
        JsonServer { uuid, created }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = server_table)]
pub struct InsertServer {
    pub uuid: ServerUuid,
    pub created: DateTime,
}

impl Default for InsertServer {
    fn default() -> Self {
        Self {
            uuid: ServerUuid::new(),
            created: DateTime::now(),
        }
    }
}
