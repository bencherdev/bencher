use reports::Testbed;

use crate::cli::clap::CliTestbed;

impl Into<Testbed> for CliTestbed {
    fn into(self) -> Testbed {
        Testbed {
            name: self.testbed,
            os: self.os,
            os_version: self.os_version,
            cpu: self.cpu,
            ram: self.ram,
            disk: self.disk,
            arch: self.arch,
        }
    }
}
