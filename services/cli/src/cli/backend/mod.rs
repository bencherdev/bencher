use crate::cli::clap::CliBackend;

use reports::Report;

use crate::BencherError;

mod testbed;

use testbed::Testbed;

#[derive(Debug)]
pub struct Backend {
    url: Option<String>,
    email: String,
    project: Option<String>,
    testbed: Testbed,
}

impl From<CliBackend> for Backend {
    fn from(backend: CliBackend) -> Self {
        Self {
            url: backend.url,
            email: backend.email,
            project: backend.project,
            testbed: Testbed::from(backend.testbed),
        }
    }
}

impl Backend {
    pub async fn send(&self, report: Report) -> Result<(), BencherError> {
        // let body = reqwest::get("https://www.rust-lang.org")
        //     .await?
        //     .text()
        //     .await?;
        println!("TODO");
        Ok(())
    }
}
