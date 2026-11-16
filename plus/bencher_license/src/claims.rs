use bencher_json::{
    DateTime, Entitlements, Jwt, OrganizationUuid, organization::plan::JsonLicense,
};
use chrono::Duration;
use jsonwebtoken::dangerous::insecure_decode_claims;
use serde::{Deserialize, Serialize};

use crate::{LicenseError, audience::Audience};

/// How long a license stays valid after the end of its paid term.
pub const GRACE_PERIOD: Duration = Duration::days(30);
/// How far ahead of a server's clock a license may be issued,
/// so a server whose clock runs a little behind Bencher Cloud's takes a license signed moments ago.
pub const ISSUED_AT_LEEWAY: Duration = Duration::minutes(5);

/// The claims of a Bencher Plus Enterprise license.
///
/// A license in the earlier format names a plan level and metric entitlements instead of
/// Benchmark Series and a term, so it does not deserialize into these claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Audience
    pub aud: Audience,
    /// Expiration: the end of the paid term plus the grace period (as a UTC timestamp)
    pub exp: i64,
    /// Issued at (as a UTC timestamp)
    pub iat: i64,
    /// Issuer
    pub iss: String,
    /// Subject: the Self-Hosted organization the license is for
    pub sub: OrganizationUuid,
    /// The Benchmark Series the contract covers
    pub ser: Entitlements,
    /// The end of the paid term (as a UTC timestamp)
    pub trm: i64,
}

impl Claims {
    pub(crate) fn new(
        organization: OrganizationUuid,
        series: Entitlements,
        issued_at: DateTime,
        term_end: DateTime,
        issuer: &str,
    ) -> Result<Self, LicenseError> {
        if term_end.timestamp() <= issued_at.timestamp() {
            return Err(LicenseError::Term {
                issued_at,
                term_end,
            });
        }
        Ok(Self {
            aud: Audience::Bencher,
            exp: (term_end + GRACE_PERIOD).timestamp(),
            iat: issued_at.timestamp(),
            iss: issuer.to_owned(),
            sub: organization,
            ser: series,
            trm: term_end.timestamp(),
        })
    }

    /// Refuse a license that is not valid at `now`: one issued after `now`, past the leeway,
    /// or one at or past its expiration.
    pub(crate) fn check_time(&self, now: DateTime) -> Result<(), LicenseError> {
        if (now + ISSUED_AT_LEEWAY).timestamp() < self.iat {
            return Err(LicenseError::NotYetIssued {
                issued_at: self.issued_at(),
                now,
            });
        }
        if self.exp <= now.timestamp() {
            return Err(LicenseError::Expired {
                expiration: self.expiration(),
                now,
            });
        }
        Ok(())
    }

    pub fn organization(&self) -> OrganizationUuid {
        self.sub
    }

    pub fn series(&self) -> Entitlements {
        self.ser
    }

    pub fn issued_at(&self) -> DateTime {
        timestamp(self.iat)
    }

    pub fn term_end(&self) -> DateTime {
        timestamp(self.trm)
    }

    pub fn expiration(&self) -> DateTime {
        timestamp(self.exp)
    }

    /// The license JSON for `key`, the license these claims are of.
    pub fn into_json(self, key: Jwt) -> JsonLicense {
        JsonLicense {
            key,
            organization: self.organization(),
            series: self.series(),
            issued_at: self.issued_at(),
            term_end: self.term_end(),
            expiration: self.expiration(),
        }
    }

    /// The claims of `license` with nothing checked, not even its key: only for a license that
    /// Bencher Cloud keeps in its own database, whose key may no longer verify after the signing
    /// key changed. Never for a license that a user gives.
    pub fn read_unverified(license: &Jwt) -> Result<Self, LicenseError> {
        Ok(insecure_decode_claims(license.as_ref())?)
    }

    /// Whether `now` is past the end of the paid term, in the grace before the expiration.
    pub fn in_grace(&self, now: DateTime) -> bool {
        self.trm <= now.timestamp()
    }

    /// Whether a license for `series` Benchmark Series, whose term ends at `term_end`, replaces
    /// this one: its term ends later, or ends at the same time and covers another count.
    /// A license never moves an organization back a term.
    pub fn replaced_by(&self, series: Entitlements, term_end: DateTime) -> bool {
        let term_end = term_end.timestamp();
        term_end > self.trm || (term_end == self.trm && series != self.ser)
    }
}

fn timestamp(timestamp: i64) -> DateTime {
    let date_time = DateTime::try_from(timestamp);
    debug_assert!(date_time.is_ok(), "License timestamp is invalid");
    date_time.unwrap_or_default()
}
