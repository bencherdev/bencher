#![cfg(feature = "plus")]

use std::sync::Arc;

use bencher_json::{DateTime, JsonServer, ServerUuid};
use chrono::{Duration, NaiveTime, Utc};
use diesel::RunQueryDsl;
use dropshot::HttpError;
use once_cell::sync::Lazy;

use crate::{
    config::plus::StatsSettings, context::DbConnection, error::resource_conflict_err, schema,
    schema::server as server_table, util::fn_get::fn_get,
};

crate::util::typed_id::typed_id!(ServerId);

const SERVER_ID: ServerId = ServerId(1);

#[derive(Debug, Clone, diesel::Queryable)]
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

    pub async fn spawn_stats(
        self,
        conn: Arc<tokio::sync::Mutex<DbConnection>>,
        stats_settings: StatsSettings,
    ) {
        tokio::spawn(async move {
            loop {
                let now = Utc::now().naive_utc().time();
                let sleep_time = match now.cmp(&stats_settings.offset) {
                    std::cmp::Ordering::Less => stats_settings.offset - now,
                    std::cmp::Ordering::Equal => Duration::days(1),
                    std::cmp::Ordering::Greater => {
                        Duration::days(1) - (now - stats_settings.offset)
                    },
                }
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(24 * 60 * 60));
                tokio::time::sleep(sleep_time).await;

                println!("Hello, world!");
            }
        });
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
