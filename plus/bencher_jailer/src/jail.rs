//! Main jail orchestration.
//!
//! This module ties together namespaces, cgroups, chroot, and rlimits
//! to create a complete jail environment.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use nix::unistd::{chdir, execve, fork, ForkResult};

use crate::cgroup::CgroupManager;
use crate::chroot::{do_pivot_root, mount_essential_filesystems, setup_jail_root};
use crate::config::JailConfig;
use crate::error::JailerError;
use crate::namespace::{create_namespaces, set_hostname, set_no_new_privs, setup_uid_gid_mapping};
use crate::rlimit::apply_rlimits;

/// A jail that encapsulates all isolation mechanisms.
pub struct Jail {
    config: JailConfig,
    cgroup: Option<CgroupManager>,
}

impl Jail {
    /// Create a new jail with the given configuration.
    pub fn new(config: JailConfig) -> Result<Self, JailerError> {
        Ok(Self {
            config,
            cgroup: None,
        })
    }

    /// Run the jailed process.
    ///
    /// This function:
    /// 1. Sets up cgroups
    /// 2. Creates namespaces
    /// 3. Sets up the jail root filesystem
    /// 4. Forks
    /// 5. In child: pivot_root, drop privileges, exec
    /// 6. In parent: wait for child
    ///
    /// # Returns
    ///
    /// The exit status of the jailed process.
    pub fn run(&mut self) -> Result<i32, JailerError> {
        // Step 1: Set up cgroup (before fork, so we can add child to it)
        if crate::cgroup::is_cgroup_v2_available() {
            let mut cgroup = CgroupManager::new(&self.config.id)?;
            cgroup.apply_limits(&self.config.limits)?;
            self.cgroup = Some(cgroup);
        }

        // Step 2: Set up jail root filesystem
        let jail_root = Path::new(self.config.jail_root.as_str());
        setup_jail_root(jail_root, &self.config.bind_mounts)?;

        // Step 3: Fork
        // SAFETY: fork() is safe to call. We handle both parent and child cases.
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: add child to cgroup and wait
                if let Some(ref cgroup) = self.cgroup {
                    cgroup.add_pid(child.as_raw() as u32)?;
                }

                // Wait for child
                let status = nix::sys::wait::waitpid(child, None)
                    .map_err(|e| JailerError::Exec(format!("waitpid failed: {e}")))?;

                // Extract exit code
                match status {
                    nix::sys::wait::WaitStatus::Exited(_, code) => Ok(code),
                    nix::sys::wait::WaitStatus::Signaled(_, sig, _) => {
                        // Return 128 + signal number (shell convention)
                        Ok(128 + sig as i32)
                    }
                    _ => Ok(1),
                }
            }
            Ok(ForkResult::Child) => {
                // Child: set up jail and exec
                if let Err(e) = self.run_child() {
                    eprintln!("Jail setup failed: {e}");
                    std::process::exit(1);
                }
                // run_child should not return (it calls execve)
                std::process::exit(1);
            }
            Err(e) => Err(JailerError::Exec(format!("fork failed: {e}"))),
        }
    }

    /// Run in the child process after fork.
    fn run_child(&self) -> Result<(), JailerError> {
        let jail_root = Path::new(self.config.jail_root.as_str());

        // Create namespaces
        create_namespaces(&self.config.namespaces)?;

        // Set up UID/GID mapping if using user namespace
        if self.config.namespaces.user {
            setup_uid_gid_mapping(self.config.uid, self.config.gid)?;
        }

        // Set hostname in UTS namespace
        if self.config.namespaces.uts {
            set_hostname(&format!("jail-{}", self.config.id))?;
        }

        // Mount essential filesystems
        mount_essential_filesystems(jail_root)?;

        // Pivot root
        do_pivot_root(jail_root)?;

        // Change to working directory
        chdir(self.config.workdir.as_str())
            .map_err(|e| JailerError::Chroot(format!("chdir to workdir failed: {e}")))?;

        // Apply rlimits
        apply_rlimits(&self.config.limits)?;

        // Set no new privileges
        set_no_new_privs()?;

        // Drop capabilities (all of them)
        drop_all_capabilities()?;

        // Prepare exec arguments
        let exec_path = CString::new(self.config.exec_path.as_str())
            .map_err(|e| JailerError::Exec(format!("invalid exec path: {e}")))?;

        let mut args: Vec<CString> = vec![exec_path.clone()];
        for arg in &self.config.exec_args {
            args.push(
                CString::new(arg.as_str())
                    .map_err(|e| JailerError::Exec(format!("invalid argument: {e}")))?,
            );
        }

        // Prepare environment
        let mut env: Vec<CString> = Vec::new();
        for (key, value) in &self.config.env {
            env.push(
                CString::new(format!("{key}={value}"))
                    .map_err(|e| JailerError::Exec(format!("invalid env var: {e}")))?,
            );
        }

        // Add minimal default environment
        if !self.config.env.iter().any(|(k, _)| k == "PATH") {
            env.push(CString::new("PATH=/usr/bin:/bin").expect("static string"));
        }
        if !self.config.env.iter().any(|(k, _)| k == "HOME") {
            env.push(CString::new("HOME=/").expect("static string"));
        }

        // Exec the target
        execve(&exec_path, &args, &env)
            .map_err(|e| JailerError::Exec(format!("execve failed: {e}")))?;

        // execve doesn't return on success
        unreachable!()
    }
}

/// Drop all Linux capabilities.
fn drop_all_capabilities() -> Result<(), JailerError> {
    use caps::{clear, CapSet};

    // Clear all capability sets
    clear(None, CapSet::Effective)
        .map_err(|e| JailerError::Privileges(format!("failed to clear effective caps: {e}")))?;
    clear(None, CapSet::Permitted)
        .map_err(|e| JailerError::Privileges(format!("failed to clear permitted caps: {e}")))?;
    clear(None, CapSet::Inheritable)
        .map_err(|e| JailerError::Privileges(format!("failed to clear inheritable caps: {e}")))?;
    clear(None, CapSet::Ambient)
        .map_err(|e| JailerError::Privileges(format!("failed to clear ambient caps: {e}")))?;

    Ok(())
}
