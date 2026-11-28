use bencher_json::{DateTime, Entitlements, Jwt, OrganizationUuid};
use clap::{Parser, Subcommand};

#[derive(Subcommand, Debug)]
pub enum TaskLicense {
    /// Generate a license key
    Generate(TaskLicenseGenerate),
    /// Validate a license key
    Validate(TaskLicenseValidate),
}

#[derive(Parser, Debug)]
pub struct TaskLicenseGenerate {
    /// Self-Hosted organization UUID
    pub organization: OrganizationUuid,

    /// License pem
    #[clap(long, env = "BENCHER_LICENSE_PEM")]
    pub pem: String,

    /// The Benchmark Series the license covers
    #[clap(long)]
    pub series: Entitlements,

    /// The end of the paid term, in RFC 3339 (`2027-01-01T00:00:00Z`).
    /// The license expires 30 days after it.
    #[clap(long, value_parser = parse_date_time)]
    pub term_end: DateTime,
}

#[derive(Parser, Debug)]
pub struct TaskLicenseValidate {
    pub license: Jwt,

    /// License pem
    #[clap(long, env = "BENCHER_LICENSE_PEM")]
    pub pem: String,
}

fn parse_date_time(date_time: &str) -> Result<DateTime, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(date_time.to_owned()))
}

#[cfg(test)]
mod tests {
    use bencher_json::DateTime;
    use clap::Parser as _;

    use super::TaskLicenseGenerate;

    // Fails if the term end is not read as RFC 3339, or the Benchmark Series are not required.
    #[test]
    fn generate_takes_the_series_and_the_term_end() {
        let organization = "00000000-0000-4000-8000-000000000001";
        let generate = TaskLicenseGenerate::try_parse_from([
            "generate",
            organization,
            "--pem",
            "pem",
            "--series",
            "250",
            "--term-end",
            "2027-01-01T00:00:00Z",
        ])
        .unwrap();
        assert_eq!(u32::from(generate.series), 250);
        assert_eq!(
            generate.term_end,
            DateTime::try_from(1_798_761_600).unwrap()
        );

        TaskLicenseGenerate::try_parse_from([
            "generate",
            organization,
            "--pem",
            "pem",
            "--term-end",
            "2027-01-01T00:00:00Z",
        ])
        .unwrap_err();
        TaskLicenseGenerate::try_parse_from([
            "generate",
            organization,
            "--pem",
            "pem",
            "--series",
            "250",
            "--term-end",
            "1798761600",
        ])
        .unwrap_err();
    }
}
