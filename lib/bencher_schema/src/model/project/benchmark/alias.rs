use std::collections::{HashMap, HashSet};

use bencher_json::BenchmarkName;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::BenchmarkId;
use crate::{
    context::DbConnection,
    error::{BencherResource, bad_request_error, resource_conflict_error},
    model::project::ProjectId,
    schema::{benchmark, benchmark_alias},
};

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = benchmark_alias)]
pub struct InsertBenchmarkAlias {
    pub project_id: ProjectId,
    pub benchmark_id: BenchmarkId,
    pub alias: BenchmarkName,
}

/// Validates alias strings for a benchmark: no duplicates, none equal the primary name, and no
/// collision with another benchmark's `name` or `alias` in the same project.
pub fn validate_benchmark_aliases_uniqueness(
    conn: &mut DbConnection,
    project_id: ProjectId,
    exclude_benchmark_id: Option<BenchmarkId>,
    primary_name: &BenchmarkName,
    aliases: &[BenchmarkName],
) -> Result<(), HttpError> {
    if aliases.is_empty() {
        return Ok(());
    }

    let mut seen = HashSet::new();
    for alias in aliases {
        let key = alias.as_ref();
        if !seen.insert(key) {
            return Err(bad_request_error(format!(
                "Duplicate benchmark alias: {alias}"
            )));
        }
        if alias == primary_name {
            return Err(bad_request_error(
                "A benchmark alias must not match the benchmark's primary name",
            ));
        }
    }

    let mut name_query = benchmark::table
        .filter(benchmark::project_id.eq(project_id))
        .filter(benchmark::name.eq_any(aliases))
        .into_boxed();
    if let Some(ex) = exclude_benchmark_id {
        name_query = name_query.filter(benchmark::id.ne(ex));
    }
    let conflicting_names: Vec<BenchmarkName> = name_query
        .select(benchmark::name)
        .load(conn)
        .map_err(|e| resource_conflict_error(BencherResource::BenchmarkAlias, (project_id,), e))?;

    let mut alias_query = benchmark_alias::table
        .filter(benchmark_alias::project_id.eq(project_id))
        .filter(benchmark_alias::alias.eq_any(aliases))
        .into_boxed();
    if let Some(ex) = exclude_benchmark_id {
        alias_query = alias_query.filter(benchmark_alias::benchmark_id.ne(ex));
    }
    let conflicting_aliases: Vec<BenchmarkName> = alias_query
        .select(benchmark_alias::alias)
        .load(conn)
        .map_err(|e| resource_conflict_error(BencherResource::BenchmarkAlias, (project_id,), e))?;

    let name_set: HashSet<String> = conflicting_names
        .into_iter()
        .map(|n| n.as_ref().to_owned())
        .collect();
    let alias_set: HashSet<String> = conflicting_aliases
        .into_iter()
        .map(|a| a.as_ref().to_owned())
        .collect();

    for alias in aliases {
        let key = alias.as_ref();
        if name_set.contains(key) {
            return Err(resource_conflict_error(
                BencherResource::BenchmarkAlias,
                (project_id, alias.clone()),
                format!("Conflicts with another benchmark's name: {alias}"),
            ));
        }
        if alias_set.contains(key) {
            return Err(resource_conflict_error(
                BencherResource::BenchmarkAlias,
                (project_id, alias.clone()),
                format!("Conflicts with another benchmark's alias: {alias}"),
            ));
        }
    }
    Ok(())
}

/// Deletes all aliases for a benchmark and inserts the given list. Caller must validate first.
pub fn replace_benchmark_aliases(
    conn: &mut DbConnection,
    project_id: ProjectId,
    benchmark_id: BenchmarkId,
    aliases: &[BenchmarkName],
) -> diesel::QueryResult<()> {
    diesel::delete(benchmark_alias::table.filter(benchmark_alias::benchmark_id.eq(benchmark_id)))
        .execute(conn)?;

    if aliases.is_empty() {
        return Ok(());
    }

    let rows: Vec<InsertBenchmarkAlias> = aliases
        .iter()
        .map(|alias| InsertBenchmarkAlias {
            project_id,
            benchmark_id,
            alias: alias.clone(),
        })
        .collect();
    diesel::insert_into(benchmark_alias::table)
        .values(&rows)
        .execute(conn)?;
    Ok(())
}

pub fn list_aliases_for_benchmark(
    conn: &mut DbConnection,
    benchmark_id: BenchmarkId,
) -> diesel::QueryResult<Vec<BenchmarkName>> {
    benchmark_alias::table
        .filter(benchmark_alias::benchmark_id.eq(benchmark_id))
        .order(benchmark_alias::id.asc())
        .select(benchmark_alias::alias)
        .load(conn)
}

pub fn aliases_by_benchmark_id(
    conn: &mut DbConnection,
    project_id: ProjectId,
    benchmark_ids: &[BenchmarkId],
) -> diesel::QueryResult<HashMap<BenchmarkId, Vec<BenchmarkName>>> {
    if benchmark_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(BenchmarkId, BenchmarkName)> = benchmark_alias::table
        .filter(benchmark_alias::project_id.eq(project_id))
        .filter(benchmark_alias::benchmark_id.eq_any(benchmark_ids))
        .order(benchmark_alias::id.asc())
        .select((benchmark_alias::benchmark_id, benchmark_alias::alias))
        .load(conn)?;

    let mut map: HashMap<BenchmarkId, Vec<BenchmarkName>> = HashMap::new();
    for (bid, alias) in rows {
        map.entry(bid).or_default().push(alias);
    }
    Ok(map)
}
