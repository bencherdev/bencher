use std::{fs::Permissions, os::unix::fs::PermissionsExt, process::Command};

use camino::Utf8PathBuf;

use crate::{
    parser::{TaskDeb, TaskMan},
    BENCHER_VERSION,
};

use super::man::Man;

#[derive(Debug)]
pub struct Deb {
    bin: Utf8PathBuf,
    dir: Utf8PathBuf,
    arch: String,
}

impl TryFrom<TaskDeb> for Deb {
    type Error = anyhow::Error;

    fn try_from(deb: TaskDeb) -> Result<Self, Self::Error> {
        let TaskDeb { bin, dir, arch } = deb;
        Ok(Self { bin, dir, arch })
    }
}

impl Deb {
    pub fn exec(&self) -> anyhow::Result<()> {
        #[allow(clippy::expect_used)]
        let deb_path = self.dir.join(
            self.bin
                .file_name()
                .expect("bin path should have a file name"),
        );
        let bin_path = deb_path.join("usr/local/bin");
        let bencher_path = bin_path.join("bencher");

        std::fs::create_dir_all(&bin_path)?;
        std::fs::set_permissions(&self.bin, Permissions::from_mode(0o755))?;
        std::fs::copy(&self.bin, bencher_path)?;

        let debian_path = deb_path.join("DEBIAN");
        std::fs::create_dir_all(&debian_path)?;

        let control_path = debian_path.join("control");
        let control = format!("Package: bencher\nVersion: {BENCHER_VERSION}\nArchitecture: {arch}\nMaintainer: Bencher <info@bencher.dev>\nDescription: Continuous Benchmarking\n", arch = self.arch);
        std::fs::write(control_path, control)?;

        let man = Man::try_from(TaskMan {
            path: debian_path.clone(),
            name: None,
        })?;
        man.exec()?;

        let _dpkg = Command::new("dpkg-deb")
            .args(["-Zxz", "--build", "--root-owner-group", deb_path.as_ref()])
            .output()?;

        Ok(())
    }
}
