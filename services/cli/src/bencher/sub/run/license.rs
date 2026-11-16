//! What `bencher run` says about the Bencher Plus Enterprise license of the report's organization.

use bencher_json::organization::plan::{JsonLicenseGrace, JsonLicenseOver, JsonReportLicense};

/// One warning for each thing the report response says about the organization's license.
pub fn license_warnings(license: &JsonReportLicense) -> Vec<String> {
    let JsonReportLicense { over, grace } = *license;
    let over = over.map(
        |JsonLicenseOver {
             active_series,
             series,
         }| {
            format!(
                "Warning: The organization has {active_series} Benchmark Series active in the last 30 days, more than the {series} its Bencher Plus Enterprise license covers."
            )
        },
    );
    let grace = grace.map(
        |JsonLicenseGrace {
             term_end,
             expiration,
         }| {
            format!(
                "Warning: The term of the organization's Bencher Plus Enterprise license ended {term_end}, and the license expires {expiration}."
            )
        },
    );
    over.into_iter().chain(grace).collect()
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        DateTime, Entitlements,
        organization::plan::{JsonLicenseGrace, JsonLicenseOver, JsonReportLicense},
    };

    use super::license_warnings;

    // Fails if a warning drops the count, the license's count, or a date, or if a license
    // warning that says nothing prints a line.
    #[test]
    fn license_warnings_name_the_counts_and_the_dates() {
        let over = JsonLicenseOver {
            active_series: 12,
            series: Entitlements::try_from(10).unwrap(),
        };
        let term_end = DateTime::try_from(1_800_000_000).unwrap();
        let expiration = DateTime::try_from(1_800_000_000 + 30 * 24 * 60 * 60).unwrap();
        let grace = JsonLicenseGrace {
            term_end,
            expiration,
        };
        let over_line = "Warning: The organization has 12 Benchmark Series active in the last 30 days, more than the 10 its Bencher Plus Enterprise license covers.";
        let grace_line = format!(
            "Warning: The term of the organization's Bencher Plus Enterprise license ended {term_end}, and the license expires {expiration}."
        );

        assert_eq!(
            license_warnings(&JsonReportLicense {
                over: None,
                grace: None,
            }),
            Vec::<String>::new()
        );
        assert_eq!(
            license_warnings(&JsonReportLicense {
                over: Some(over),
                grace: None,
            }),
            vec![over_line.to_owned()]
        );
        assert_eq!(
            license_warnings(&JsonReportLicense {
                over: None,
                grace: Some(grace),
            }),
            vec![grace_line.clone()]
        );
        assert_eq!(
            license_warnings(&JsonReportLicense {
                over: Some(over),
                grace: Some(grace),
            }),
            vec![over_line.to_owned(), grace_line]
        );
    }
}
