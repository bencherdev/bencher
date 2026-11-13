#![cfg(feature = "plus")]

//! A Self-Hosted organization's license: whether it is valid, its organization's active
//! Benchmark Series against the ones it covers, and the highest count seen in each paid term.

use bencher_json::{
    DateTime, Jwt, OrganizationUuid,
    organization::{
        plan::{JsonLicense, JsonLicenseGrace, JsonLicenseOver, JsonReportLicense},
        usage::JsonLicensePeak,
    },
};
use bencher_license::{LicenseClaims, LicenseError, Licensor};
use chrono::Duration;
use diesel::{
    ExpressionMethods as _, NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _,
};
use dropshot::HttpError;
use slog::Logger;

use crate::{
    ApiContext,
    context::DbConnection,
    error::issue_error,
    model::{
        organization::{OrganizationId, QueryOrganization},
        project::series::count_active,
    },
    schema::{self, license_peak as license_peak_table},
    write_transaction,
};

/// How far back a Self-Hosted license counts its organization's active Benchmark Series.
pub const ACTIVE_SERIES_WINDOW: Duration = Duration::days(30);

/// A license that an organization holds and that is valid for it.
#[derive(Debug, Clone)]
pub struct OrganizationLicense {
    pub key: Jwt,
    pub claims: LicenseClaims,
}

impl OrganizationLicense {
    /// The organization's license, `None` when it holds none,
    /// or an error when the one it holds is not valid for it at `now`.
    pub fn get(
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        now: DateTime,
    ) -> Result<Option<Self>, LicenseError> {
        // It is important that we check the organization license and NOT the plan license
        // The organization license is the one that is actually in use on Self-Hosted
        // The plan license is simply there to keep track of the license on Bencher Cloud only
        let Some(key) = query_organization.license.clone() else {
            return Ok(None);
        };
        let claims = licensor.validate_organization(&key, query_organization.uuid, now)?;
        Ok(Some(Self { key, claims }))
    }

    /// The organization's license when it holds one that is valid for it at `now`.
    /// A license that is not valid reads as none.
    pub fn get_valid(
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        now: DateTime,
    ) -> Option<Self> {
        Self::get(licensor, query_organization, now).ok().flatten()
    }

    pub fn into_json(self) -> JsonLicense {
        let Self { key, claims } = self;
        claims.into_json(key)
    }

    /// After a report to the organization: count its active Benchmark Series, raise the peak of
    /// the license's term, and return what the report response says about the license.
    ///
    /// Best effort: the report is already committed, so a failure to count or to raise the
    /// peak is logged and never refuses it.
    pub async fn after_report(
        &self,
        log: &Logger,
        context: &ApiContext,
        organization_id: OrganizationId,
    ) -> Option<JsonReportLicense> {
        let now = context.clock.now();
        let active_series = match self
            .count_and_raise_peak(context, organization_id, now)
            .await
        {
            Ok(active_series) => Some(active_series),
            Err(e) => {
                slog::warn!(
                    log,
                    "Failed to count the active series of licensed organization ({organization_id}): {e}"
                );
                None
            },
        };
        report_license(&self.claims, active_series, now)
    }

    async fn count_and_raise_peak(
        &self,
        context: &ApiContext,
        organization_id: OrganizationId,
        now: DateTime,
    ) -> Result<u32, HttpError> {
        let term_end = self.claims.term_end();
        let mut conn = context.database.get_public_conn().await?;
        let active_series = count_active_series(&mut conn, organization_id, now)?;
        let count = SeriesCount::from(active_series);
        // Most reports leave the peak alone, so they never wait on the writer.
        let stored =
            QueryLicensePeak::count(&mut conn, organization_id, term_end).map_err(peak_error)?;
        let held = held_license(&mut conn, organization_id).map_err(peak_error)?;
        drop(conn);
        if !raises_peak(stored, held.as_ref(), &self.key, count) {
            return Ok(active_series);
        }

        // Read again under the write lock, which a license update also takes,
        // so a count under a license the organization no longer holds writes nothing.
        write_transaction!(context, |conn| {
            let stored = QueryLicensePeak::count(conn, organization_id, term_end)?;
            let held = held_license(conn, organization_id)?;
            if raises_peak(stored, held.as_ref(), &self.key, count) {
                QueryLicensePeak {
                    organization_id,
                    term_end,
                    active_series: count,
                    seen: now,
                }
                .upsert(conn)?;
            }
            diesel::QueryResult::Ok(())
        })
        .map_err(peak_error)?;
        Ok(active_series)
    }
}

/// The organization's Benchmark Series active in the [`ACTIVE_SERIES_WINDOW`] that ends at `now`,
/// in every visibility.
pub fn count_active_series(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    now: DateTime,
) -> Result<u32, HttpError> {
    count_active(conn, organization_id, now - ACTIVE_SERIES_WINDOW, now)
}

/// Whether any organization on the server holds a license that is valid for it at `now`, its
/// grace included. The server features of Bencher Plus are on while one does.
pub fn any_valid_license(
    conn: &mut DbConnection,
    licensor: &Licensor,
    now: DateTime,
) -> Result<bool, HttpError> {
    // Intentionally includes soft-deleted organizations, as a license belongs to the server
    let licenses = schema::organization::table
        .filter(schema::organization::license.is_not_null())
        .select((
            schema::organization::uuid,
            schema::organization::license.assume_not_null(),
        ))
        .load::<(OrganizationUuid, Jwt)>(conn)
        .map_err(|e| {
            issue_error(
                "Failed to load the licenses on the server",
                "Failed to load the licenses of every organization on the server.",
                e,
            )
        })?;
    Ok(any_valid(
        licensor,
        licenses.iter().map(|(uuid, license)| (*uuid, license)),
        now,
    ))
}

fn any_valid<'a>(
    licensor: &Licensor,
    licenses: impl IntoIterator<Item = (OrganizationUuid, &'a Jwt)>,
    now: DateTime,
) -> bool {
    licenses.into_iter().any(|(organization, license)| {
        licensor
            .validate_organization(license, organization, now)
            .is_ok()
    })
}

/// What a report response says about the license: that the organization's `active_series`
/// are above the ones the license covers, and that the license is in its grace at `now`.
/// `None` when neither holds. Without a count, it can only say the second.
fn report_license(
    claims: &LicenseClaims,
    active_series: Option<u32>,
    now: DateTime,
) -> Option<JsonReportLicense> {
    let series = claims.series();
    let over = active_series
        .filter(|&active_series| active_series > u32::from(series))
        .map(|active_series| JsonLicenseOver {
            active_series,
            series,
        });
    let grace = claims.in_grace(now).then(|| JsonLicenseGrace {
        term_end: claims.term_end(),
        expiration: claims.expiration(),
    });
    (over.is_some() || grace.is_some()).then_some(JsonReportLicense { over, grace })
}

/// Whether a count of `active_series` under `license` raises the peak of the license's term,
/// given the term's stored count and the license the organization holds now. A count under a
/// license that the organization no longer holds raises nothing.
fn raises_peak(
    stored: Option<SeriesCount>,
    held: Option<&Jwt>,
    license: &Jwt,
    active_series: SeriesCount,
) -> bool {
    held == Some(license) && stored.is_none_or(|stored| active_series > stored)
}

/// The term end of the license the organization holds, when it is valid for it at `now`.
///
/// Read on the caller's connection, so under the write lock it is the license the write replaces.
pub fn held_term_end(
    conn: &mut DbConnection,
    licensor: &Licensor,
    query_organization: &QueryOrganization,
    now: DateTime,
) -> diesel::QueryResult<Option<DateTime>> {
    Ok(
        held_license(conn, query_organization.id)?.and_then(|license| {
            licensor
                .validate_organization(&license, query_organization.uuid, now)
                .ok()
                .map(|claims| claims.term_end())
        }),
    )
}

/// The license the organization holds now, valid or not.
fn held_license(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
) -> diesel::QueryResult<Option<Jwt>> {
    schema::organization::table
        .filter(schema::organization::id.eq(organization_id))
        .select(schema::organization::license)
        .first(conn)
}

/// The highest count of active Benchmark Series seen in one paid term of an organization's
/// license. Every term keeps its row.
#[derive(Debug, diesel::Queryable, diesel::Selectable, diesel::Insertable)]
#[diesel(table_name = license_peak_table)]
pub struct QueryLicensePeak {
    pub organization_id: OrganizationId,
    pub term_end: DateTime,
    pub active_series: SeriesCount,
    pub seen: DateTime,
}

impl QueryLicensePeak {
    /// The peak count of the term that ends at `term_end`.
    fn count(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        term_end: DateTime,
    ) -> diesel::QueryResult<Option<SeriesCount>> {
        license_peak_table::table
            .filter(license_peak_table::organization_id.eq(organization_id))
            .filter(license_peak_table::term_end.eq(term_end))
            .select(license_peak_table::active_series)
            .first(conn)
            .optional()
    }

    /// The peak of the term that ends at `term_end`, `None` when no report in it has been counted.
    pub fn for_term(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        term_end: DateTime,
    ) -> Result<Option<JsonLicensePeak>, HttpError> {
        Ok(license_peak_table::table
            .filter(license_peak_table::organization_id.eq(organization_id))
            .filter(license_peak_table::term_end.eq(term_end))
            .first::<Self>(conn)
            .optional()
            .map_err(peak_error)?
            .map(Self::into_json))
    }

    /// The peak of the latest term that ends before `before`, or of the latest term of all when
    /// there is no `before`.
    pub fn previous(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        before: Option<DateTime>,
    ) -> Result<Option<JsonLicensePeak>, HttpError> {
        let mut query = license_peak_table::table
            .filter(license_peak_table::organization_id.eq(organization_id))
            .into_boxed();
        if let Some(before) = before {
            query = query.filter(license_peak_table::term_end.lt(before));
        }
        Ok(query
            .order(license_peak_table::term_end.desc())
            .first::<Self>(conn)
            .optional()
            .map_err(peak_error)?
            .map(Self::into_json))
    }

    fn upsert(&self, conn: &mut DbConnection) -> diesel::QueryResult<()> {
        diesel::insert_into(license_peak_table::table)
            .values(self)
            .on_conflict((
                license_peak_table::organization_id,
                license_peak_table::term_end,
            ))
            .do_update()
            .set((
                license_peak_table::active_series.eq(self.active_series),
                license_peak_table::seen.eq(self.seen),
            ))
            .execute(conn)?;
        Ok(())
    }

    fn into_json(self) -> JsonLicensePeak {
        let Self {
            organization_id: _,
            term_end,
            active_series,
            seen,
        } = self;
        JsonLicensePeak {
            term_end,
            active_series: active_series.into(),
            seen,
        }
    }
}

fn peak_error(error: diesel::result::Error) -> HttpError {
    issue_error(
        "Failed to read or write the license peak",
        "Failed to read or write the license peak of an organization.",
        error,
    )
}

/// A count of active Benchmark Series, stored as an integer.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, diesel::FromSqlRow, diesel::AsExpression,
)]
#[diesel(sql_type = diesel::sql_types::Integer)]
pub struct SeriesCount(u32);

impl From<u32> for SeriesCount {
    fn from(count: u32) -> Self {
        Self(count)
    }
}

impl From<SeriesCount> for u32 {
    fn from(count: SeriesCount) -> Self {
        count.0
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for SeriesCount
where
    DB: diesel::backend::Backend,
    for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
        + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        out.set_value(i32::try_from(self.0)?);
        Ok(diesel::serialize::IsNull::No)
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for SeriesCount
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self(u32::try_from(i32::from_sql(bytes)?)?))
    }
}

#[cfg(test)]
impl OrganizationLicense {
    /// A license valid at [`DateTime::TEST`].
    pub(crate) fn test() -> Self {
        const TEST_LICENSE_KEY: &str =
            include_str!("../../../../../plus/bencher_license/src/test/private.pem");
        let organization = OrganizationUuid::new();
        let key: bencher_json::Secret = TEST_LICENSE_KEY.parse().unwrap();
        let licensor = Licensor::bencher_cloud(&key).unwrap();
        let license = licensor
            .new_license(
                organization,
                bencher_json::Entitlements::MIN,
                DateTime::TEST,
                DateTime::TEST + Duration::days(365),
            )
            .unwrap();
        let claims = licensor
            .validate_organization(&license, organization, DateTime::TEST)
            .unwrap();
        Self {
            key: license,
            claims,
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        DateTime, Entitlements, Jwt, OrganizationUuid, Secret,
        organization::plan::{JsonLicenseGrace, JsonLicenseOver, JsonReportLicense},
    };
    use bencher_license::Licensor;
    use chrono::Duration;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use pretty_assertions::assert_eq;

    use super::{
        QueryLicensePeak, SeriesCount, any_valid, any_valid_license, raises_peak, report_license,
    };
    use crate::{
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    // Debug builds trust this key's public half for Self-Hosted licenses.
    const TEST_LICENSE_KEY: &str =
        include_str!("../../../../../plus/bencher_license/src/test/private.pem");
    // No build trusts this key's public half.
    const FOREIGN_LICENSE_KEY: &str =
        include_str!("../../../../../plus/bencher_license/src/test/foreign.pem");

    fn now() -> DateTime {
        DateTime::TEST
    }

    fn sign(
        key: &str,
        organization: OrganizationUuid,
        issued_at: DateTime,
        term_end: DateTime,
    ) -> Jwt {
        let key: Secret = key.parse().unwrap();
        Licensor::bencher_cloud(&key)
            .unwrap()
            .new_license(organization, Entitlements::MIN, issued_at, term_end)
            .unwrap()
    }

    fn valid(organization: OrganizationUuid) -> Jwt {
        sign(
            TEST_LICENSE_KEY,
            organization,
            now(),
            now() + Duration::days(365),
        )
    }

    fn in_grace(organization: OrganizationUuid) -> Jwt {
        sign(
            TEST_LICENSE_KEY,
            organization,
            now() - Duration::days(40),
            now() - Duration::days(1),
        )
    }

    fn expired(organization: OrganizationUuid) -> Jwt {
        sign(
            TEST_LICENSE_KEY,
            organization,
            now() - Duration::days(400),
            now() - Duration::days(31),
        )
    }

    fn foreign(organization: OrganizationUuid) -> Jwt {
        sign(
            FOREIGN_LICENSE_KEY,
            organization,
            now(),
            now() + Duration::days(365),
        )
    }

    // Fails if a server with no valid license turns its features on, or one with a single valid
    // license, its grace included, among invalid ones leaves them off.
    #[test]
    fn any_valid_license_turns_the_server_features_on() {
        let licensor = Licensor::self_hosted().unwrap();
        let organization = OrganizationUuid::new();
        let any = |licenses: &[(OrganizationUuid, Jwt)]| {
            any_valid(
                &licensor,
                licenses.iter().map(|(uuid, license)| (*uuid, license)),
                now(),
            )
        };

        assert!(!any(&[]));
        assert!(!any(&[(organization, expired(organization))]));
        assert!(!any(&[(organization, foreign(organization))]));
        // A license for another organization is not valid for this one.
        assert!(!any(&[(organization, valid(OrganizationUuid::new()))]));

        assert!(any(&[(organization, valid(organization))]));
        assert!(any(&[(organization, in_grace(organization))]));
        let other = OrganizationUuid::new();
        assert!(any(&[
            (organization, expired(organization)),
            (other, in_grace(other)),
        ]));
    }

    // Fails if a count does not start its term's peak, a higher count does not raise it, or an
    // equal or lower count rewrites it.
    #[test]
    fn a_higher_count_raises_the_terms_peak() {
        let license = valid(OrganizationUuid::new());
        let held = Some(&license);
        let count = SeriesCount::from;

        assert!(raises_peak(None, held, &license, count(3)));
        assert!(raises_peak(Some(count(3)), held, &license, count(4)));
        assert!(!raises_peak(Some(count(3)), held, &license, count(3)));
        assert!(!raises_peak(Some(count(3)), held, &license, count(2)));
    }

    // Fails if a count taken under a license the organization has since replaced or removed
    // writes a peak.
    #[test]
    fn a_license_no_longer_held_raises_nothing() {
        let organization = OrganizationUuid::new();
        let license = valid(organization);
        let renewed = in_grace(organization);
        let count = SeriesCount::from(5);

        assert!(raises_peak(None, Some(&license), &license, count));
        assert!(!raises_peak(None, Some(&renewed), &license, count));
        assert!(!raises_peak(None, None, &license, count));
    }

    // Fails if the previous term's peak is not the latest term before the current one, or, with
    // no current term, the latest term of all.
    #[test]
    fn the_previous_peak_is_the_latest_earlier_term() {
        let mut conn = setup_test_db();
        let organization_id = create_base_entities(&mut conn).organization_id;
        let term = |days| now() + Duration::days(days);
        for (term_end, active_series) in [(term(30), 7), (term(60), 3), (term(90), 5)] {
            QueryLicensePeak {
                organization_id,
                term_end,
                active_series: SeriesCount::from(active_series),
                seen: term_end - Duration::days(1),
            }
            .upsert(&mut conn)
            .unwrap();
        }
        let mut previous = |before| {
            QueryLicensePeak::previous(&mut conn, organization_id, before)
                .unwrap()
                .map(|peak| (peak.term_end, peak.active_series))
        };

        assert_eq!(previous(Some(term(90))), Some((term(60), 3)));
        assert_eq!(previous(Some(term(60))), Some((term(30), 7)));
        assert_eq!(previous(Some(term(30))), None);
        assert_eq!(previous(None), Some((term(90), 5)));
    }

    // Fails if a valid license on a soft-deleted organization no longer turns the server features
    // on, as a license belongs to the server.
    #[test]
    fn any_valid_license_counts_a_soft_deleted_organization() {
        let mut conn = setup_test_db();
        let organization_id = create_base_entities(&mut conn).organization_id;
        let organization: OrganizationUuid = schema::organization::table
            .find(organization_id)
            .select(schema::organization::uuid)
            .first(&mut conn)
            .unwrap();
        let licensor = Licensor::self_hosted().unwrap();
        diesel::update(schema::organization::table.find(organization_id))
            .set(schema::organization::deleted.eq(Some(now())))
            .execute(&mut conn)
            .unwrap();
        assert!(!any_valid_license(&mut conn, &licensor, now()).unwrap());

        diesel::update(schema::organization::table.find(organization_id))
            .set(schema::organization::license.eq(valid(organization)))
            .execute(&mut conn)
            .unwrap();
        assert!(any_valid_license(&mut conn, &licensor, now()).unwrap());
    }

    // Fails if a report at the license's count in its term warns, or one over the license or in
    // the grace does not warn with its numbers and dates.
    #[test]
    fn report_license_warns_over_the_series_and_in_the_grace() {
        let licensor = Licensor::self_hosted().unwrap();
        let organization = OrganizationUuid::new();
        let term_end = now() + Duration::days(10);
        let claims = licensor
            .validate(
                &sign(TEST_LICENSE_KEY, organization, now(), term_end),
                now(),
            )
            .unwrap();
        let series = Entitlements::MIN;
        let over = Some(JsonLicenseOver {
            active_series: 2,
            series,
        });
        let grace = Some(JsonLicenseGrace {
            term_end,
            expiration: term_end + Duration::days(30),
        });

        assert_eq!(report_license(&claims, Some(1), now()), None);
        assert_eq!(
            report_license(&claims, Some(2), now()),
            Some(JsonReportLicense { over, grace: None })
        );
        assert_eq!(
            report_license(&claims, Some(1), term_end),
            Some(JsonReportLicense { over: None, grace })
        );
        assert_eq!(
            report_license(&claims, Some(2), term_end),
            Some(JsonReportLicense { over, grace })
        );
        // Without a count, only the grace is known.
        assert_eq!(report_license(&claims, None, now()), None);
        assert_eq!(
            report_license(&claims, None, term_end),
            Some(JsonReportLicense { over: None, grace })
        );
    }
}
