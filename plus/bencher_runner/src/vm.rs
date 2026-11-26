//! Linux VM execution — runs benchmarks in Firecracker microVMs.

use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};
use slog::{Logger, info};

use crate::JobDeadline;
use crate::error::RunnerError;
use crate::firecracker::{ResultsDrive, refuse_cancelled};
use crate::jail::{HostPreparation, JailDir, JailPaths, StateDir, VmId, chroot, netns, state};
use crate::run::{PulledImage, RunOutput};

/// An image unpacked once, with init and its config in place, which every
/// iteration builds a new guest disk from. The guest writes only to its disk,
/// so the tree stays as it was unpacked.
pub(crate) struct GuestTree {
    image: PulledImage,
    rootfs: Utf8PathBuf,
}

impl GuestTree {
    pub(crate) fn prepare(
        log: &Logger,
        config: &crate::Config,
        host: &mut HostPreparation,
        cancel_flag: Option<&AtomicBool>,
        deadline: JobDeadline,
    ) -> Result<Self, RunnerError> {
        // Before the image pull, so a host that cannot jail at all fails fast.
        prepare_state_dir(log, config, host)?;
        refuse_cancelled(cancel_flag)?;

        let image = PulledImage::pull(log, config, deadline)?;
        let rootfs = image.work_dir().join("rootfs");
        image.unpack(log, &rootfs)?;

        let oci_config = image.oci_config();
        info!(log, "Writing init config");
        write_init_config(
            &rootfs,
            &oci_config.command,
            &oci_config.working_dir,
            &oci_config.env,
            config.file_paths.as_deref(),
            config.max_output_size,
        )?;

        info!(log, "Installing init binary");
        install_init_binary(&rootfs)?;
        refuse_cancelled(cancel_flag)?;

        Ok(Self { image, rootfs })
    }
}

/// Execute a single benchmark run in a jailed Firecracker microVM, booted from
/// a new disk built from `tree`, which gets only what is left of `deadline`.
pub fn vm_execute(
    log: &Logger,
    config: &crate::Config,
    tree: &GuestTree,
    host: &mut HostPreparation,
    cancel_flag: Option<&AtomicBool>,
    deadline: JobDeadline,
) -> Result<RunOutput, RunnerError> {
    use crate::firecracker::run_firecracker;

    info!(log, "Executing benchmark run";
        "image" => bencher_logger::capped(&config.oci_image),
        "kernel" => config.kernel.as_ref().map_or("(system)", |p| p.as_str()),
        "vcpus" => u32::from(config.vcpus),
        "memory_mib" => config.memory.to_mib(),
        "timeout_secs" => config.timeout_secs,
    );

    let state_dir = prepare_state_dir(log, config, host)?;
    refuse_cancelled(cancel_flag)?;

    // Every job, not once per process, so a teardown this runner could not
    // finish is retried.
    state_dir.sweep(log)?;
    refuse_cancelled(cancel_flag)?;

    // Rebuilt per job rather than once per daemon lifetime: the handle lives
    // on a tmpfs and is operator visible, so it has to be self-healing.
    let netns = netns::ensure(log)?;

    // Minted before any artifact exists, because the jail root is a function of
    // the VM id and the artifacts are built inside it.
    let vm_id = VmId::new();
    let jail_dir = JailDir::create(log, &state_dir, &vm_id)?;
    let jail = JailPaths::new(jail_dir.root());
    info!(log, "Jail built"; "jail_root" => jail.root().as_str());

    // The tree, with the staged binaries beside it, the jail, with the kernel
    // and both drives in it, and the blobs the pull wrote.
    let written = [
        tree.image.work_dir(),
        jail_dir.root(),
        tree.image.store_dir(),
    ];
    build_flush_boot(
        || {
            // Everything Firecracker reads has to be inside the chroot, so the
            // kernel lands in the jail root whatever its source.
            let kernel_dest = jail.kernel().host().as_path();
            if let Some(kernel) = &config.kernel {
                copy_file(log, kernel, kernel_dest)?;
            } else if crate::kernel::KERNEL_BUNDLED {
                crate::kernel::write_kernel_to_file(kernel_dest)?;
                info!(log, "Extracted the bundled kernel"; "path" => kernel_dest.as_str());
            } else {
                copy_file(log, &find_kernel()?, kernel_dest)?;
            }

            // Step 6: Create the ext4 rootfs directly in the jail root
            let rootfs_dest = jail.rootfs().host().as_path();
            info!(log, "Creating the ext4 rootfs";
                "path" => rootfs_dest.as_str(),
                "disk_mib" => config.disk.to_mib(),
            );
            bencher_rootfs::create_ext4_with_size(&tree.rootfs, rootfs_dest, config.disk.to_mib())?;

            // The jailer does not chown what the runner placed in the chroot:
            // the rootfs is written by Firecracker so it is given away, while
            // the kernel is only read so it stays root-owned.
            chroot::chown_to_jail(rootfs_dest, config.jail_user)?;
            chroot::grant_jail_read(kernel_dest)?;

            // Step 7: Build the Firecracker config
            build_firecracker_config(
                log,
                config,
                tree.image.work_dir(),
                vm_id,
                &state_dir,
                jail,
                netns,
            )
        },
        || Ok(crate::flush::flush_filesystems(log, &written)?),
        // Step 8: Run the microVM
        |fc_config| {
            refuse_cancelled(cancel_flag)?;
            Ok(run_firecracker(log, &fc_config, cancel_flag, deadline)?)
        },
    )
}

/// Builds the files a VM boots from, flushes them to disk, and only then
/// boots it, so no writeback of them runs while the benchmark does. Each step
/// is a parameter, so a test can stand in for each.
fn build_flush_boot<C, B, F, R>(build: B, flush: F, boot: R) -> Result<RunOutput, RunnerError>
where
    B: FnOnce() -> Result<C, RunnerError>,
    F: FnOnce() -> Result<(), RunnerError>,
    R: FnOnce(C) -> Result<RunOutput, RunnerError>,
{
    let files = build()?;
    flush()?;
    boot(files)
}

fn prepare_state_dir(
    log: &Logger,
    config: &crate::Config,
    host: &mut HostPreparation,
) -> Result<StateDir, RunnerError> {
    let state_dir = StateDir::new(config.state_dir.clone())?;
    host.ensure(log, state_dir.path(), config.jail_user)?;
    state_dir.refuse_unusable_mount()?;
    Ok(state_dir)
}

/// Build the Firecracker job config: stage the binaries, make the results
/// drive, and convert types.
fn build_firecracker_config(
    log: &Logger,
    config: &crate::Config,
    work_dir: &Utf8Path,
    vm_id: VmId,
    state_dir: &StateDir,
    jail: JailPaths,
    netns: Utf8PathBuf,
) -> Result<crate::firecracker::FirecrackerJobConfig, RunnerError> {
    // Staged outside the jail under a fixed name: the jailer copies
    // `--exec-file` in itself, rejects a hardlinked one, and derives the chroot
    // layout from its base name.
    let firecracker_bin = work_dir.join(state::EXEC_FILE_NAME);
    if crate::firecracker_bin::FIRECRACKER_BUNDLED {
        crate::firecracker_bin::write_firecracker_to_file(&firecracker_bin)?;
        info!(log, "Extracted the bundled firecracker"; "path" => firecracker_bin.as_str());
    } else {
        copy_binary(log, &find_firecracker_binary()?, &firecracker_bin)?;
    }

    // The jailer runs outside the chroot and is never copied into it, so it
    // can be used wherever it is found.
    let jailer_bin = if crate::jailer_bin::JAILER_BUNDLED {
        let jailer_dest = work_dir.join("jailer");
        crate::jailer_bin::write_jailer_to_file(&jailer_dest)?;
        info!(log, "Extracted the bundled jailer"; "path" => jailer_dest.as_str());
        jailer_dest
    } else {
        find_jailer_binary()?
    };

    info!(log, "Launching the jailed Firecracker microVM");
    let vcpus = u8::try_from(u32::from(config.vcpus)).map_err(|_err| {
        crate::error::ConfigError::OutOfRange {
            name: "vCPU count",
            value: config.vcpus.to_string(),
            range: "0-255",
        }
    })?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Practical memory fits in u32 MiB for Firecracker"
    )]
    let memory_mib = config.memory.to_mib() as u32;

    // Like the rootfs, and before any guest code runs, so the host holds the
    // drive it made.
    let results = ResultsDrive::create(
        jail.results().host().as_path(),
        config.jail_user,
        config.max_output_size,
    )?;

    Ok(crate::firecracker::FirecrackerJobConfig {
        firecracker_bin,
        jailer_bin,
        vm_id,
        jail,
        jail_user: config.jail_user,
        chroot_base_dir: state_dir.chroot_base(),
        netns,
        vcpus,
        memory_mib,
        boot_args: config.kernel_cmdline.clone(),
        cpu_layout: config.cpu_layout.clone(),
        log_level: config.sandbox_log_level,
        max_file_count: config.max_file_count,
        max_content_size: config.max_content_size,
        results,
    })
}

fn copy_file(log: &Logger, src: &Utf8Path, dest: &Utf8Path) -> Result<(), RunnerError> {
    std::fs::copy(src, dest).map_err(|e| crate::error::ConfigError::CopyFile {
        src: src.to_owned(),
        dest: dest.to_owned(),
        source: e,
    })?;
    info!(log, "Copied"; "src" => src.as_str(), "dest" => dest.as_str());
    Ok(())
}

/// Forces mode 0755, because `fs::copy` carries over whatever mode the host
/// gave the source.
fn copy_binary(log: &Logger, src: &Utf8Path, dest: &Utf8Path) -> Result<(), RunnerError> {
    use std::os::unix::fs::PermissionsExt as _;

    copy_file(log, src, dest)?;
    let chmod = |source| crate::error::ConfigError::ChmodBinary {
        path: dest.to_owned(),
        source,
    };
    let mut perms = std::fs::metadata(dest).map_err(chmod)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(dest, perms).map_err(chmod)?;
    Ok(())
}

/// Write the init config for the VM.
///
/// This creates `/etc/bencher/config.json` which is read by `bencher-init`.
fn write_init_config(
    rootfs: &Utf8Path,
    command: &[String],
    workdir: &str,
    env: &[(String, String)],
    file_paths: Option<&[Utf8PathBuf]>,
    max_output_size: usize,
) -> Result<(), RunnerError> {
    use std::fs;

    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Build the config JSON
    let config = serde_json::json!({
        "command": command,
        "workdir": workdir,
        "env": env,
        "file_paths": file_paths,
        "max_output_size": max_output_size,
    });

    let config_path = config_dir.join("config.json");
    let config_str =
        serde_json::to_string_pretty(&config).map_err(crate::error::ConfigError::Serialize)?;
    fs::write(&config_path, config_str)?;

    Ok(())
}

/// Install the bencher-init binary into the rootfs at /init.
///
/// Uses the bundled init binary if available, otherwise falls back to searching on disk.
fn install_init_binary(rootfs: &Utf8Path) -> Result<(), RunnerError> {
    use crate::init;
    use std::os::unix::fs::PermissionsExt as _;

    let dest_path = rootfs.join("init");

    if init::INIT_BUNDLED {
        // Use the bundled init binary
        init::write_init_to_file(&dest_path)?;
    } else {
        // Fall back to searching for the binary on disk
        let init_binary = find_init_binary()?;

        std::fs::copy(&init_binary, &dest_path).map_err(|e| {
            crate::error::ConfigError::CopyInit {
                src: init_binary.clone(),
                dest: dest_path.clone(),
                source: e,
            }
        })?;

        // Make it executable
        let mut perms = std::fs::metadata(&dest_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest_path, perms)?;
    }

    Ok(())
}

/// Beside the runner first, so a self-contained install finds its own copy
/// before the host's.
fn binary_candidates(name: &str) -> impl Iterator<Item = Utf8PathBuf> {
    [
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
            .and_then(|path| Utf8PathBuf::try_from(path).ok()),
        Some(Utf8PathBuf::from(format!("/usr/local/bin/{name}"))),
        Some(Utf8PathBuf::from(format!("/usr/bin/{name}"))),
    ]
    .into_iter()
    .flatten()
}

/// A candidate that cannot be stat'ed is passed over, since the search is a
/// list of guesses and only finding nothing is worth reporting.
fn find_binary(name: &str, hint: &str) -> Result<Utf8PathBuf, RunnerError> {
    binary_candidates(name)
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            crate::error::ConfigError::BinaryNotFound {
                name: name.to_owned(),
                hint: hint.to_owned(),
            }
            .into()
        })
}

const FIRECRACKER_RELEASES: &str =
    "Install from: https://github.com/firecracker-microvm/firecracker/releases";

/// Find the bencher-init binary on disk (fallback when not bundled).
fn find_init_binary() -> Result<Utf8PathBuf, RunnerError> {
    find_binary("bencher-init", "Build with: cargo build -p bencher_init")
}

/// Find the Firecracker binary on the system.
fn find_firecracker_binary() -> Result<Utf8PathBuf, RunnerError> {
    find_binary("firecracker", FIRECRACKER_RELEASES)
}

fn find_jailer_binary() -> Result<Utf8PathBuf, RunnerError> {
    find_binary("jailer", FIRECRACKER_RELEASES)
}

/// Find the kernel image on the system.
fn find_kernel() -> Result<Utf8PathBuf, RunnerError> {
    let candidates = [
        // Bencher's shared location
        "/usr/local/share/bencher/vmlinux",
        // Next to the current executable
    ];

    for candidate in candidates {
        if Utf8Path::new(candidate).exists() {
            return Ok(Utf8PathBuf::from(candidate));
        }
    }

    // Try next to the current executable
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let kernel = parent.join("vmlinux");
        if kernel.exists()
            && let Some(path) = kernel.to_str()
        {
            return Ok(Utf8PathBuf::from(path));
        }
    }

    Err(crate::error::ConfigError::BinaryNotFound {
        name: "vmlinux".to_owned(),
        hint: "Place at /usr/local/share/bencher/vmlinux".to_owned(),
    }
    .into())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::time::Duration;

    use super::*;
    use crate::error::ExecutionError;

    fn powered_off() -> RunOutput {
        RunOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            output_files: None,
            duration: Duration::ZERO,
        }
    }

    #[test]
    fn the_flush_runs_after_the_disk_is_built_and_before_the_vm_boots() {
        // Kills a flush that is skipped or runs once the VM is up, which leaves
        // the writeback of the tree and the disk to run during the benchmark.
        let steps = RefCell::new(Vec::new());

        build_flush_boot(
            || {
                steps.borrow_mut().push("build");
                Ok(())
            },
            || {
                steps.borrow_mut().push("flush");
                Ok(())
            },
            |()| {
                steps.borrow_mut().push("boot");
                Ok(powered_off())
            },
        )
        .unwrap();

        assert_eq!(steps.into_inner(), ["build", "flush", "boot"]);
    }

    #[test]
    fn a_failed_flush_fails_the_run_before_the_vm_boots() {
        let booted = RefCell::new(false);

        let result = build_flush_boot(
            || Ok(()),
            || {
                Err(ExecutionError::Flush {
                    path: Utf8PathBuf::from("/var/lib/bencher"),
                    source: std::io::Error::from_raw_os_error(libc::EIO),
                }
                .into())
            },
            |()| {
                *booted.borrow_mut() = true;
                Ok(powered_off())
            },
        );

        let Err(RunnerError::Execution(ExecutionError::Flush { path, .. })) = &result else {
            panic!("the run must fail with its flush error, got: {result:?}");
        };
        assert_eq!(path, "/var/lib/bencher");
        assert!(!booted.into_inner(), "a VM booted after its flush failed");
    }
}
