use std::fmt;

/// A callback request sealed to one job: the nonce, then the ciphertext and its tag.
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Binary))]
pub struct SealedRequest(Vec<u8>);

impl SealedRequest {
    pub(crate) fn new(sealed: Vec<u8>) -> Self {
        Self(sealed)
    }
}

impl fmt::Debug for SealedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedRequest")
            .field("len", &self.0.len())
            .finish()
    }
}

impl AsRef<[u8]> for SealedRequest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(feature = "db")]
mod db {
    use diesel::{
        backend::Backend,
        deserialize::{self, FromSql},
        serialize::{self, IsNull, Output, ToSql},
        sql_types::Binary,
        sqlite::Sqlite,
    };

    use super::SealedRequest;

    impl ToSql<Binary, Sqlite> for SealedRequest {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
            out.set_value(self.0.as_slice());
            Ok(IsNull::No)
        }
    }

    impl<DB> FromSql<Binary, DB> for SealedRequest
    where
        DB: Backend,
        Vec<u8>: FromSql<Binary, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
            Vec::<u8>::from_sql(bytes).map(Self)
        }
    }
}
