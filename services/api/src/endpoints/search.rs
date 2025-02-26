use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub struct Search(String);

#[allow(clippy::absolute_paths)]
impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for Search
where
    DB: diesel::backend::Backend,
    for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
        + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        // https://docs.rs/diesel/latest/diesel/serialize/trait.ToSql.html#examples
        out.set_value(format!("%{}%", self.0));
        Ok(diesel::serialize::IsNull::No)
    }
}
