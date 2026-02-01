use clap::Parser;

const TEST_USERNAME: &str = "muriel.bagge@nowhere.com";
const TEST_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4";

/// Run OCI Distribution Spec conformance tests
#[derive(Parser, Debug)]
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

    /// Username for OCI authentication (email address)
    #[clap(long, default_value = TEST_USERNAME)]
    pub username: String,

    /// Password for OCI authentication (API token)
    #[clap(long, default_value = TEST_API_TOKEN)]
    pub password: String,
}
