use std::collections::{HashMap, HashSet};

use bencher_json::{CommitTitle, GitHash};
#[cfg(feature = "plus")]
use bencher_json::{DateTime, project::github::LinkedRepositoryStatus};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, NullableExpressionMethods as _,
    QueryDsl as _, RunQueryDsl as _, result::QueryResult,
};
#[cfg(feature = "plus")]
use diesel::{JoinOnDsl as _, dsl::exists};

use super::ProjectId;
#[cfg(feature = "plus")]
use crate::schema::commit as commit_table;
use crate::{context::DbConnection, macros::sql::lower_nullable, schema};

/// Each hash binds one parameter, so one lookup binds at most this many, well under SQLite's
/// limit on bound parameters.
const TITLE_LOOKUP: usize = 10_000;

/// Where a version's commit joins it: on the version's project and its hash in lower case,
/// which the commit's unique index looks up.
#[diesel::dsl::auto_type]
pub fn of_version() -> _ {
    schema::commit::project_id
        .eq(schema::version::project_id)
        .and(
            schema::commit::hash
                .nullable()
                .eq(lower_nullable(schema::version::hash)),
        )
}

/// The title of each of the project's hashes that has one, by its hash in lower case: one query
/// for every hash of a response, and one more for each further 10,000 distinct hashes.
pub fn titles<'a, I>(
    conn: &mut DbConnection,
    project_id: ProjectId,
    hashes: I,
) -> QueryResult<HashMap<GitHash, CommitTitle>>
where
    I: IntoIterator<Item = &'a GitHash>,
{
    let hashes = hashes
        .into_iter()
        .map(GitHash::to_ascii_lowercase)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut titles = HashMap::new();
    for batch in hashes.chunks(TITLE_LOOKUP) {
        titles.extend(
            schema::commit::table
                .filter(schema::commit::project_id.eq(project_id))
                .filter(schema::commit::hash.eq_any(batch))
                .filter(schema::commit::title.is_not_null())
                .select((
                    schema::commit::hash,
                    schema::commit::title.assume_not_null(),
                ))
                .load::<(GitHash, CommitTitle)>(conn)?,
        );
    }
    Ok(titles)
}

/// A commit of a project, by its full hash in lower case, as GitHub answered for it.
#[cfg(feature = "plus")]
#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = commit_table)]
pub struct InsertCommit {
    pub project_id: ProjectId,
    pub hash: GitHash,
    /// None when GitHub has no commit with the hash in the linked repository.
    pub title: Option<CommitTitle>,
    pub created: DateTime,
}

#[cfg(feature = "plus")]
impl InsertCommit {
    /// Store each commit the project has no row for. A stored row never changes.
    pub fn insert_new(conn: &mut DbConnection, commits: &[Self]) -> QueryResult<usize> {
        diesel::insert_or_ignore_into(schema::commit::table)
            .values(commits)
            .execute(conn)
    }
}

/// Whether the project's report with `hash` should give it to the commit title task: the project
/// has a connected link, and no row for the hash.
#[cfg(feature = "plus")]
pub fn wants_title(
    conn: &mut DbConnection,
    project_id: ProjectId,
    hash: &GitHash,
) -> QueryResult<bool> {
    let connected = schema::linked_repository::table
        .filter(schema::linked_repository::project_id.eq(project_id))
        .filter(schema::linked_repository::status.eq(LinkedRepositoryStatus::Connected));
    let stored = schema::commit::table
        .filter(schema::commit::project_id.eq(project_id))
        .filter(schema::commit::hash.eq(hash.to_ascii_lowercase()));
    diesel::select(exists(connected).and(diesel::dsl::not(exists(stored)))).get_result(conn)
}

/// Each of `hashes` in lower case that the project has no row for, once, in the order given.
#[cfg(feature = "plus")]
pub fn unstored(
    conn: &mut DbConnection,
    project_id: ProjectId,
    hashes: &[GitHash],
) -> QueryResult<Vec<GitHash>> {
    let mut seen = HashSet::new();
    let hashes = hashes
        .iter()
        .map(GitHash::to_ascii_lowercase)
        .filter(|hash| seen.insert(hash.clone()))
        .collect::<Vec<_>>();
    let stored = schema::commit::table
        .filter(schema::commit::project_id.eq(project_id))
        .filter(schema::commit::hash.eq_any(&hashes))
        .select(schema::commit::hash)
        .load::<GitHash>(conn)?
        .into_iter()
        .collect::<HashSet<_>>();
    Ok(hashes
        .into_iter()
        .filter(|hash| !stored.contains(hash))
        .collect())
}

/// The hash in lower case of every report of the project that started at or after `since` with no
/// row, newest report first, each once.
#[cfg(feature = "plus")]
pub fn recent_unstored(
    conn: &mut DbConnection,
    project_id: ProjectId,
    since: DateTime,
) -> QueryResult<Vec<GitHash>> {
    let mut seen = HashSet::new();
    Ok(schema::report::table
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .left_join(schema::commit::table.on(of_version()))
        .filter(schema::report::project_id.eq(project_id))
        .filter(schema::report::start_time.ge(since))
        // Implied by the start, and what bounds the project's end time index from below.
        .filter(schema::report::end_time.ge(since))
        .filter(schema::version::hash.is_not_null())
        .filter(schema::commit::id.nullable().is_null())
        .order((schema::report::start_time.desc(), schema::report::id.desc()))
        .select(lower_nullable(schema::version::hash))
        .load::<Option<GitHash>>(conn)?
        .into_iter()
        .flatten()
        .filter(|hash| seen.insert(hash.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, GitHash};
    use diesel::{Connection as _, ExpressionMethods as _, RunQueryDsl as _};
    use pretty_assertions::assert_eq;

    use super::{TITLE_LOOKUP, titles};
    use crate::{
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    // Kills a lookup that reads only its first chunk of hashes.
    #[test]
    fn titles_reads_every_chunk_of_hashes() {
        let mut conn = setup_test_db();
        let project_id = create_base_entities(&mut conn).project_id;
        let hashes = (0..=TITLE_LOOKUP)
            .map(|n| format!("{n:0>40x}").parse::<GitHash>().unwrap())
            .collect::<Vec<_>>();
        conn.transaction(|conn| {
            for hash in &hashes {
                diesel::insert_into(schema::commit::table)
                    .values((
                        schema::commit::project_id.eq(project_id),
                        schema::commit::hash.eq(hash),
                        schema::commit::title.eq(Some(format!("Title {hash}"))),
                        schema::commit::created.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;
            }
            diesel::QueryResult::Ok(())
        })
        .unwrap();

        let found = titles(&mut conn, project_id, &hashes).unwrap();

        assert_eq!(found.len(), TITLE_LOOKUP + 1);
        for hash in &hashes {
            assert_eq!(
                found.get(hash).map(AsRef::as_ref),
                Some(format!("Title {hash}").as_str())
            );
        }
    }
}
