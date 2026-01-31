use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name = "test-oci", about = "Run OCI Distribution Spec conformance tests")]
pub struct TaskOci {
    /// API URL to test against
    #[clap(long, default_value = "http://localhost:61016")]
    pub api_url: String,

    /// Repository namespace for tests
    #[clap(long, default_value = "test/repo")]
    pub namespace: String,

    /// Cross-mount namespace for tests
    #[clap(long, default_value = "test/other")]
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
}
