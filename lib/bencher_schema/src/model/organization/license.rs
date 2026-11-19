#![cfg(feature = "plus")]

use bencher_json::{DateTime, Jwt, OrganizationUuid, PlanLevel};
use bencher_license::Licensor;
use diesel::{
    ExpressionMethods as _, NullableExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{context::DbConnection, error::issue_error, schema};

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
            .validate_organization(license, organization)
            .is_ok_and(|token_data| {
                token_data.claims.level() >= PlanLevel::Enterprise
                    && now.timestamp() <= token_data.claims.exp
            })
    })
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, Entitlements, Jwt, OrganizationUuid, PlanLevel, Secret};
    use bencher_license::Licensor;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use super::{any_valid, any_valid_license};
    use crate::{
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    // Debug builds trust this key's public half for Self-Hosted licenses.
    const TEST_LICENSE_KEY: &str =
        include_str!("../../../../../plus/bencher_license/src/test/private.pem");

    fn sign(organization: OrganizationUuid, level: PlanLevel) -> Jwt {
        let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
        Licensor::bencher_cloud(&key)
            .unwrap()
            .new_annual_license(organization, level, Entitlements::MIN)
            .unwrap()
    }

    /// The moment before and the moment after `license` expires.
    fn around_expiration(licensor: &Licensor, license: &Jwt) -> (DateTime, DateTime) {
        let expiration = licensor.validate(license).unwrap().claims.exp;
        (
            DateTime::try_from(expiration - 1).unwrap(),
            DateTime::try_from(expiration + 1).unwrap(),
        )
    }

    // Kills a server with no valid license that turns its features on, one with a single valid
    // Enterprise license among invalid ones that leaves them off, a license read as valid after it
    // expires at `now`, below the Enterprise level, for another organization, or with a broken
    // signature.
    #[test]
    fn any_valid_license_turns_the_server_features_on() {
        let licensor = Licensor::self_hosted().unwrap();
        let organization = OrganizationUuid::new();
        let enterprise = sign(organization, PlanLevel::Enterprise);
        let (before, after) = around_expiration(&licensor, &enterprise);
        let any = |licenses: &[(OrganizationUuid, Jwt)], now| {
            any_valid(
                &licensor,
                licenses.iter().map(|(uuid, license)| (*uuid, license)),
                now,
            )
        };
        let (signed, signature) = enterprise.as_ref().rsplit_once('.').unwrap();
        let mut signature = signature.chars();
        let first = if signature.next() == Some('A') {
            'B'
        } else {
            'A'
        };
        let broken: Jwt = format!("{signed}.{first}{}", signature.as_str())
            .parse()
            .unwrap();

        assert!(!any(&[], before));
        assert!(!any(&[(organization, enterprise.clone())], after));
        assert!(!any(
            &[(organization, sign(organization, PlanLevel::Team))],
            before
        ));
        assert!(!any(
            &[(
                organization,
                sign(OrganizationUuid::new(), PlanLevel::Enterprise)
            )],
            before
        ));
        assert!(!any(&[(organization, broken)], before));

        assert!(any(&[(organization, enterprise)], before));
        let other = OrganizationUuid::new();
        assert!(any(
            &[
                (organization, sign(organization, PlanLevel::Team)),
                (other, sign(other, PlanLevel::Enterprise)),
            ],
            before
        ));
    }

    // Kills a valid license on a soft-deleted organization that no longer turns the server
    // features on, as a license belongs to the server, and a server with no license that does.
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
        let license = sign(organization, PlanLevel::Enterprise);
        let (now, _after) = around_expiration(&licensor, &license);
        diesel::update(schema::organization::table.find(organization_id))
            .set(schema::organization::deleted.eq(Some(DateTime::TEST)))
            .execute(&mut conn)
            .unwrap();
        assert!(!any_valid_license(&mut conn, &licensor, now).unwrap());

        diesel::update(schema::organization::table.find(organization_id))
            .set(schema::organization::license.eq(license))
            .execute(&mut conn)
            .unwrap();
        assert!(any_valid_license(&mut conn, &licensor, now).unwrap());
    }
}
