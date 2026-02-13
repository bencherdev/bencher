use bencher_json::JsonJobConfig;

/// Newtype wrapping `JsonJobConfig` for Diesel serialization.
/// Stored as a JSON text column in `SQLite`.
#[derive(Debug, Clone, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub struct JobConfig(pub JsonJobConfig);

impl From<JobConfig> for JsonJobConfig {
    fn from(config: JobConfig) -> Self {
        config.0
    }
}

impl std::ops::Deref for JobConfig {
    type Target = JsonJobConfig;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for JobConfig
where
    DB: diesel::backend::Backend,
    for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
        + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        let json = serde_json::to_string(&self.0)?;
        out.set_value(json);
        Ok(diesel::serialize::IsNull::No)
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for JobConfig
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let json_str = String::from_sql(bytes)?;
        let config: JsonJobConfig = serde_json::from_str(&json_str)?;
        Ok(Self(config))
    }
}
