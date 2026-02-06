use clap::Parser;

const TEST_USERNAME: &str = "muriel.bagge@nowhere.com";
const TEST_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4";

pub const TEST_ADMIN_USERNAME: &str = "eustace.bagge@nowhere.com";
pub const TEST_ADMIN_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQzNjA5LCJpYXQiOjE2OTg2NzYzMTQsImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJldXN0YWNlLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.xumYID-R4waqhyjhcbSlwartbiRJ2AwngVkevLUBVCA";

/// Run OCI Distribution Spec conformance tests
#[derive(Parser, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "CLI argument struct")]
pub struct TaskOci {
    /// API URL to test against
    #[clap(long, default_value = "http://localhost:61016")]
    pub api_url: String,

    /// Repository namespace for tests
    #[clap(long, default_value = "namespace")]
    pub namespace: String,

    /// Cross-mount namespace for tests
    #[clap(long, default_value = "crossmount-namespace")]
    pub crossmount_namespace: String,

    /// Run only pull tests
    #[clap(long)]
    pub pull_only: bool,

    /// Skip building the conformance binary
    #[clap(long)]
    pub skip_build: bool,

    /// Enable debug output from conformance tests
    #[clap(long)]
    pub debug: bool,

    /// Directory for conformance test output
    #[clap(long, default_value = "./oci-conformance-results")]
    pub output_dir: String,

    /// Path to distribution-spec clone (will clone if not exists)
    #[clap(long, default_value = "./distribution-spec")]
    pub spec_dir: String,

    /// Use admin credentials for OCI authentication (required for pull operations)
    #[clap(long)]
    pub admin: bool,

    /// Username for OCI authentication (email address)
    /// Ignored if --admin is set
    #[clap(long, default_value = TEST_USERNAME)]
    pub username: String,

    /// Password for OCI authentication (API token)
    /// Ignored if --admin is set
    #[clap(long, default_value = TEST_API_TOKEN)]
    pub password: String,
}

impl TaskOci {
    /// Create a `TaskOci` configured for smoke tests with the given API URL and default credentials
    pub fn for_test(api_url: &str, admin: bool) -> Self {
        let (username, password) = if admin {
            (
                TEST_ADMIN_USERNAME.to_owned(),
                TEST_ADMIN_API_TOKEN.to_owned(),
            )
        } else {
            (TEST_USERNAME.to_owned(), TEST_API_TOKEN.to_owned())
        };
        Self::for_test_with_credentials(api_url, admin, username, password)
    }

    /// Create a `TaskOci` configured for smoke tests with custom credentials
    pub fn for_test_with_credentials(
        api_url: &str,
        admin: bool,
        username: String,
        password: String,
    ) -> Self {
        Self {
            api_url: api_url.to_owned(),
            namespace: "namespace".to_owned(),
            crossmount_namespace: "crossmount-namespace".to_owned(),
            pull_only: false,
            skip_build: false,
            debug: false,
            output_dir: "./oci-conformance-results".to_owned(),
            spec_dir: "./distribution-spec".to_owned(),
            admin,
            username,
            password,
        }
    }
}
