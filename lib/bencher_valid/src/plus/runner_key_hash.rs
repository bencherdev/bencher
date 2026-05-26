use derive_more::Display;

#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct RunnerKeyHash(String);

crate::keys::api_key_hash_impl!(RunnerKeyHash, RunnerKey, error = RunnerKeyHash);

crate::keys::api_key_hash_tests!(
    RunnerKeyHash,
    RunnerKey,
    key_sample = "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
    other_sample = "bencher_runner_xY9mN2pQ7rS4tU8vW1zK5jL0fGhaB3",
);
