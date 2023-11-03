#![cfg(feature = "plus")]

use std::sync::Arc;

use bencher_json::{DateTime, JsonServer, JsonServerStats, ServerUuid, BENCHER_API_URL};
use chrono::{Duration, NaiveTime, Utc};
use diesel::RunQueryDsl;
use dropshot::HttpError;

use crate::{
    context::DbConnection, error::resource_conflict_err, schema, schema::server as server_table,
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

    pub fn spawn_stats(self, conn: Arc<tokio::sync::Mutex<DbConnection>>, offset: NaiveTime) {
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
                let Ok(json_stats) = self.get_stats(conn) else {
                    continue;
                };
                let Ok(json_stats_str) = serde_json::to_string(&json_stats) else {
                    continue;
                };
                // println!("{json_stats_str}");

                let client = reqwest::Client::new();
                let _resp = client
                    .post(BENCHER_API_URL.clone())
                    .body(json_stats_str)
                    .send()
                    .await;
                // println!("{resp:?}");
            }
        });
    }

    pub fn get_stats(self, conn: &mut DbConnection) -> Result<JsonServerStats, HttpError> {
        stats::get_stats(conn, self)
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
