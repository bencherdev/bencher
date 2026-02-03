use bencher_runner::vmm::VmmConfig;
use bencher_runner::ResourceLimits;
use camino::Utf8PathBuf;

use crate::parser::TaskVmm;

#[derive(Debug)]
pub struct Vmm {
    config: VmmConfig,
}

impl TryFrom<TaskVmm> for Vmm {
    type Error = anyhow::Error;

    fn try_from(task: TaskVmm) -> Result<Self, Self::Error> {
        Ok(Self {
            config: VmmConfig {
                jail_root: Utf8PathBuf::from(&task.jail_root),
                kernel_path: Utf8PathBuf::from(&task.kernel),
                rootfs_path: Utf8PathBuf::from(&task.rootfs),
                vsock_path: task.vsock.map(|s| Utf8PathBuf::from(&s)),
                vcpus: task.vcpus,
                memory_mib: task.memory,
                kernel_cmdline: task.cmdline,
                timeout_secs: task.timeout,
                limits: ResourceLimits::default(),
                nonce: task.nonce,
            },
        })
    }
}

impl Vmm {
    pub fn exec(self) -> anyhow::Result<()> {
        bencher_runner::run_vmm(&self.config)?;
        Ok(())
    }
}
