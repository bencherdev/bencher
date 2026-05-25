use bencher_json::{
    DateTime, JsonNewUserKey, JsonUserKey, JsonUserKeyCreated, ResourceName, UserKey, UserKeyHash,
    UserKeyUuid, UserUuid, user::key::JsonUpdateUserKey,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{BencherResource, assert_parentage, bad_request_error, resource_not_found_err},
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    schema,
    schema::user_key as user_key_table,
};

use super::{QueryUser, UserId};

crate::macros::typed_id::typed_id!(UserKeyId);

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryUserKey {
    pub id: UserKeyId,
    pub uuid: UserKeyUuid,
    pub user_id: UserId,
    pub name: ResourceName,
    pub key_hash: UserKeyHash,
    pub creation: DateTime,
    pub expiration: DateTime,
    pub revoked: Option<DateTime>,
}

impl QueryUserKey {
    fn_get!(user_key, UserKeyId);
    fn_get_id!(user_key, UserKeyId, UserKeyUuid);
    fn_get_uuid!(user_key, UserKeyId, UserKeyUuid);

    pub fn get_user_key(
        conn: &mut DbConnection,
        user_id: UserId,
        uuid: UserKeyUuid,
    ) -> Result<Self, HttpError> {
        schema::user_key::table
            .filter(schema::user_key::user_id.eq(user_id))
            .filter(schema::user_key::uuid.eq(uuid))
            .first::<QueryUserKey>(conn)
            .map_err(resource_not_found_err!(UserKey, (user_id, &uuid)))
    }

    /// Unfiltered lookup by hash. Returns the key row (if any) joined with its owning user,
    /// without filtering on `revoked`, `expiration`, or `user.locked`. The caller checks
    /// those fields in Rust so it can distinguish `NotFound`, `Revoked`, `Expired`, and
    /// `Locked` for telemetry without a second query.
    pub fn from_hash(
        conn: &mut DbConnection,
        key_hash: &UserKeyHash,
    ) -> diesel::QueryResult<(Self, QueryUser)> {
        schema::user_key::table
            .inner_join(schema::user::table)
            .filter(schema::user_key::key_hash.eq(key_hash.as_ref()))
            .select((schema::user_key::all_columns, schema::user::all_columns))
            .first::<(QueryUserKey, QueryUser)>(conn)
    }

    pub fn revoke(
        conn: &mut DbConnection,
        key_id: UserKeyId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::user_key::table
                .filter(schema::user_key::id.eq(key_id))
                .filter(schema::user_key::revoked.is_null()),
        )
        .set(schema::user_key::revoked.eq(now))
        .execute(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonUserKey, HttpError> {
        let query_user = QueryUser::get(conn, self.user_id)?;
        Ok(self.into_json_inner(query_user.uuid))
    }

    pub fn into_json_for_user(self, query_user: &QueryUser) -> JsonUserKey {
        assert_parentage(
            BencherResource::User,
            query_user.id,
            BencherResource::UserKey,
            self.user_id,
        );
        self.into_json_inner(query_user.uuid)
    }

    fn into_json_inner(self, user_uuid: UserUuid) -> JsonUserKey {
        let Self {
            id: _,
            uuid,
            user_id: _,
            name,
            key_hash: _,
            creation,
            expiration,
            revoked,
        } = self;
        JsonUserKey {
            uuid,
            user: user_uuid,
            name,
            creation,
            expiration,
            revoked,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = user_key_table)]
pub struct InsertUserKey {
    pub uuid: UserKeyUuid,
    pub user_id: UserId,
    pub name: ResourceName,
    pub key_hash: UserKeyHash,
    pub creation: DateTime,
    pub expiration: DateTime,
}

impl InsertUserKey {
    pub fn from_json(
        user_id: UserId,
        json_key: JsonNewUserKey,
        now: DateTime,
    ) -> Result<(Self, UserKey), HttpError> {
        let JsonNewUserKey { name, ttl } = json_key;

        let max_ttl = u32::MAX;
        let ttl = if let Some(ttl) = ttl {
            if ttl == 0 {
                return Err(bad_request_error("TTL must be greater than zero"));
            }
            if ttl > max_ttl {
                return Err(bad_request_error(format!(
                    "Requested TTL ({ttl}) is greater than max ({max_ttl})"
                )));
            }
            ttl
        } else {
            max_ttl
        };

        let key = UserKey::generate();
        let key_hash = UserKeyHash::from(&key);
        let creation = now;
        let expiration = creation + chrono::Duration::seconds(i64::from(ttl));

        Ok((
            Self {
                uuid: UserKeyUuid::new(),
                user_id,
                name,
                key_hash,
                creation,
                expiration,
            },
            key,
        ))
    }

    pub fn into_json(self, user_uuid: UserUuid, key: UserKey) -> JsonUserKeyCreated {
        let Self {
            uuid,
            user_id: _,
            name,
            key_hash: _,
            creation,
            expiration,
        } = self;
        JsonUserKeyCreated {
            uuid,
            user: user_uuid,
            name,
            key,
            creation,
            expiration,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = user_key_table)]
pub struct UpdateUserKey {
    pub name: Option<ResourceName>,
}

impl From<JsonUpdateUserKey> for UpdateUserKey {
    fn from(update: JsonUpdateUserKey) -> Self {
        let JsonUpdateUserKey { name } = update;
        Self { name }
    }
}
