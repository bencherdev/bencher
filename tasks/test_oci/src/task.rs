use std::fs;
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use clap::Parser as _;

use crate::parser::TaskOci;

#[derive(Debug)]
pub struct Task {
    api_url: String,
    namespace: String,
    crossmount_namespace: String,
    pull_only: bool,
    skip_build: bool,
    debug: bool,
    output_dir: String,
    spec_dir: String,
}

impl TryFrom<TaskOci> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskOci) -> Result<Self, Self::Error> {
        Ok(Self {
            api_url: task.api_url,
            namespace: task.namespace,
            crossmount_namespace: task.crossmount_namespace,
            pull_only: task.pull_only,
            skip_build: task.skip_build,
            debug: task.debug,
            output_dir: task.output_dir,
            spec_dir: task.spec_dir,
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskOci::parse().try_into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        println!("=== OCI Conformance Test Runner ===");
        println!("API URL: {}", self.api_url);
        println!("Namespace: {}", self.namespace);
        println!("Crossmount Namespace: {}", self.crossmount_namespace);
        println!();

        // Check if API is running
        self.check_api_connectivity()?;

        // Clone/update conformance tests if needed
        self.ensure_spec_cloned()?;

        // Build conformance tests if needed
        self.build_conformance_tests()?;

        // Create output directory
        fs::create_dir_all(&self.output_dir)?;

        // Run the tests
        let result = self.run_conformance_tests();

        // Copy results
        self.copy_results()?;

        println!();
        println!("=== Test Complete ===");
        println!("Results saved to: {}", self.output_dir);

        let report_path = Path::new(&self.output_dir).join("report.html");
        if report_path.exists() {
            println!("Open {} to view the detailed report", report_path.display());
        }

        result
    }

    fn check_api_connectivity(&self) -> anyhow::Result<()> {
        println!("Checking API connectivity...");

        // Parse host and port from URL
        let url = self.api_url.trim_start_matches("http://").trim_start_matches("https://");
        let addr = if url.contains(':') {
            url.split('/').next().unwrap_or("localhost:61016")
        } else {
            "localhost:61016"
        };

        // Try to connect with retries
        for i in 1..=5 {
            if TcpStream::connect(addr).is_ok() {
                println!("API is running!");
                return Ok(());
            }
            if i < 5 {
                println!("Waiting for API server... ({i}/5)");
                thread::sleep(Duration::from_secs(1));
            }
        }

        anyhow::bail!(
            "Cannot connect to API at {}\n\
             Please start the Bencher API server first:\n  \
             cargo run -p bencher_api --features plus",
            self.api_url
        )
    }

    fn ensure_spec_cloned(&self) -> anyhow::Result<()> {
        if Path::new(&self.spec_dir).exists() {
            println!("distribution-spec already cloned at {}", self.spec_dir);
            return Ok(());
        }

        println!();
        println!("Cloning OCI distribution-spec repository...");

        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "https://github.com/opencontainers/distribution-spec.git",
                &self.spec_dir,
            ])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to clone distribution-spec repository");
        }

        Ok(())
    }

    fn build_conformance_tests(&self) -> anyhow::Result<()> {
        let conformance_dir = Path::new(&self.spec_dir).join("conformance");
        let conformance_binary = conformance_dir.join("conformance.test");

        if conformance_binary.exists() && self.skip_build {
            println!("Skipping build (--skip-build specified)");
            return Ok(());
        }

        if conformance_binary.exists() {
            println!("Conformance binary already exists");
            return Ok(());
        }

        println!();
        println!("Building conformance tests...");

        let status = Command::new("go")
            .args(["test", "-c"])
            .current_dir(&conformance_dir)
            .status()?;

        if !status.success() {
            anyhow::bail!(
                "Failed to build conformance tests.\n\
                 Make sure Go 1.17+ is installed."
            );
        }

        Ok(())
    }

    fn run_conformance_tests(&self) -> anyhow::Result<()> {
        let conformance_dir = Path::new(&self.spec_dir).join("conformance");
        let conformance_binary = conformance_dir.join("conformance.test");

        if !conformance_binary.exists() {
            anyhow::bail!(
                "Conformance test binary not found at {}\n\
                 Try running without --skip-build option",
                conformance_binary.display()
            );
        }

        println!();
        println!("Running conformance tests...");
        println!("Test categories enabled:");
        if self.pull_only {
            println!("  - Pull: 1");
            println!("  - Push: 0");
            println!("  - Content Discovery: 0");
            println!("  - Content Management: 0");
        } else {
            println!("  - Pull: 1");
            println!("  - Push: 1");
            println!("  - Content Discovery: 1");
            println!("  - Content Management: 1");
        }
        println!();

        let output_file = Path::new(&self.output_dir).join("test-output.log");

        let mut cmd = Command::new(&conformance_binary);
        cmd.args(["-test.v"])
            .current_dir(&conformance_dir)
            .env("OCI_ROOT_URL", &self.api_url)
            .env("OCI_NAMESPACE", &self.namespace)
            .env("OCI_CROSSMOUNT_NAMESPACE", &self.crossmount_namespace)
            .env("OCI_DEBUG", if self.debug { "1" } else { "0" })
            .env("OCI_REPORT_DIR", &self.output_dir);

        if self.pull_only {
            cmd.env("OCI_TEST_PULL", "1")
                .env("OCI_TEST_PUSH", "0")
                .env("OCI_TEST_CONTENT_DISCOVERY", "0")
                .env("OCI_TEST_CONTENT_MANAGEMENT", "0");
        } else {
            cmd.env("OCI_TEST_PULL", "1")
                .env("OCI_TEST_PUSH", "1")
                .env("OCI_TEST_CONTENT_DISCOVERY", "1")
                .env("OCI_TEST_CONTENT_MANAGEMENT", "1");
        }

        // Capture output to file while also displaying it
        let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

        // Write output to file
        let combined_output = format!(
            "STDOUT:\n{}\n\nSTDERR:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(&output_file, &combined_output)?;

        // Print output
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));

        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("Conformance tests failed with exit code: {:?}", output.status.code())
        }
    }

    fn copy_results(&self) -> anyhow::Result<()> {
        let conformance_dir = Path::new(&self.spec_dir).join("conformance");

        // Copy report.html if it exists
        let report_src = conformance_dir.join("report.html");
        if report_src.exists() {
            let report_dst = Path::new(&self.output_dir).join("report.html");
            fs::copy(&report_src, &report_dst)?;
        }

        // Copy junit.xml if it exists
        let junit_src = conformance_dir.join("junit.xml");
        if junit_src.exists() {
            let junit_dst = Path::new(&self.output_dir).join("junit.xml");
            fs::copy(&junit_src, &junit_dst)?;
        }

        Ok(())
    }
}
