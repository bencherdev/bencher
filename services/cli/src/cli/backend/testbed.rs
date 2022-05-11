use crate::cli::clap::CliTestbed;

#[derive(Debug)]
pub struct Testbed {
    name: Option<String>,
    os: Option<String>,
    os_version: Option<String>,
    cpu: Option<String>,
    ram: Option<String>,
    disk: Option<String>,
    arch: Option<String>,
}

impl From<CliTestbed> for Testbed {
    fn from(testbed: CliTestbed) -> Self {
        Self {
            name: testbed.testbed,
            os: testbed.os,
            os_version: testbed.os_version,
            cpu: testbed.cpu,
            ram: testbed.ram,
            disk: testbed.disk,
            arch: testbed.arch,
        }
    }
}
