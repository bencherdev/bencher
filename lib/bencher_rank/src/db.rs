use crate::Rank;

impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for Rank
where
    DB: diesel::backend::Backend,
    i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for Rank
where
    DB: diesel::backend::Backend,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        i64::from_sql(bytes).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use diesel::{
        Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SqliteConnection,
    };
    use libsqlite3_sys as _;
    use pretty_assertions::assert_eq;

    use crate::Rank;

    diesel::table! {
        rank_table (rank) {
            rank -> BigInt,
        }
    }

    /// The full range of `i64` values that a `Rank` may wrap, in ascending order.
    const RANK_VALUES: [i64; 5] = [i64::MIN, -1, 0, 1, i64::MAX];

    /// Create an in-memory `SQLite` database with a single `BigInt` rank column.
    fn setup_test_db() -> SqliteConnection {
        let mut conn =
            SqliteConnection::establish(":memory:").expect("Failed to create in-memory database");
        diesel::sql_query("CREATE TABLE rank_table (rank BIGINT NOT NULL)")
            .execute(&mut conn)
            .expect("Failed to create rank_table");
        conn
    }

    #[test]
    fn rank_to_sql() {
        let mut conn = setup_test_db();
        for value in RANK_VALUES {
            diesel::insert_into(rank_table::table)
                .values(rank_table::rank.eq(Rank(value)))
                .execute(&mut conn)
                .expect("Failed to insert Rank");
        }

        let raw_values: Vec<i64> = rank_table::table
            .select(rank_table::rank)
            .order(rank_table::rank.asc())
            .load(&mut conn)
            .expect("Failed to load raw i64 values");
        assert_eq!(
            raw_values,
            RANK_VALUES.to_vec(),
            "ToSql should write the inner i64 unchanged"
        );
    }

    #[test]
    fn rank_from_sql() {
        let mut conn = setup_test_db();
        for value in RANK_VALUES {
            diesel::insert_into(rank_table::table)
                .values(rank_table::rank.eq(value))
                .execute(&mut conn)
                .expect("Failed to insert raw i64");
        }

        let ranks: Vec<Rank> = rank_table::table
            .select(rank_table::rank)
            .order(rank_table::rank.asc())
            .load(&mut conn)
            .expect("Failed to load Rank values");
        assert_eq!(
            ranks,
            RANK_VALUES.map(Rank).to_vec(),
            "FromSql should read the raw i64 into Rank unchanged"
        );
    }

    #[test]
    fn rank_round_trip() {
        let mut conn = setup_test_db();
        for value in RANK_VALUES {
            diesel::insert_into(rank_table::table)
                .values(rank_table::rank.eq(Rank(value)))
                .execute(&mut conn)
                .expect("Failed to insert Rank");
        }

        for value in RANK_VALUES {
            // Filtering on a `Rank` bind value exercises `ToSql`,
            // while reading the column back exercises `FromSql`.
            let rank: Rank = rank_table::table
                .filter(rank_table::rank.eq(Rank(value)))
                .select(rank_table::rank)
                .first(&mut conn)
                .expect("Failed to find Rank");
            assert_eq!(rank, Rank(value), "Round trip should preserve the value");
        }
    }
}
