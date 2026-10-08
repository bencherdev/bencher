//! Linux VM execution — runs benchmarks in Firecracker microVMs.

use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};
use slog::{Logger, info};

use crate::JobDeadline;
use crate::error::RunnerError;
use crate::firecracker::refuse_cancelled;
use crate::jail::{
    CgroupSurvived, HostPreparation, JailDir, JailPaths, StateDir, VmId, chroot, netns, state,
};
use crate::run::{RunOutput, prepare_oci_workspace};

/// Execute a single benchmark run in a jailed Firecracker microVM, which gets
/// only what is left of `deadline`.
pub fn vm_execute(
    log: &Logger,
    config: &crate::Config,
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

    let state_dir = StateDir::new(config.state_dir.clone())?;

    // Before the image pull, so a host that cannot jail at all fails fast.
    host.ensure(log, state_dir.path(), config.jail_user)?;
    state_dir.refuse_unusable_mount()?;
    refuse_cancelled(cancel_flag)?;

    let workspace = prepare_oci_workspace(log, config)?;
    let work_dir = &workspace.work_dir;
    let unpack_dir = &workspace.unpack_dir;
    let oci_config = workspace.oci_config;

    let command = oci_config.command;
    let working_dir = &oci_config.working_dir;
    let env = oci_config.env;

    // Write command config for the VM
    info!(log, "Writing init config");
    write_init_config(
        unpack_dir,
        &command,
        working_dir,
        &env,
        config.file_paths.as_deref(),
        config.max_output_size,
    )?;

    // Step 5: Install init binary
    info!(log, "Installing init binary");
    install_init_binary(unpack_dir)?;
    refuse_cancelled(cancel_flag)?;

    // Every job, not once per process, so a teardown this runner could not
    // finish is retried.
    state_dir.sweep(log)?;
    // An orphan in another state directory is beyond the sweep.
    crate::jail::refuse_occupied_cgroups(None)?;
    refuse_cancelled(cancel_flag)?;

    // Rebuilt per job rather than once per daemon lifetime: the handle lives
    // on a tmpfs and is operator visible, so it has to be self-healing.
    let netns = netns::ensure(log)?;

    // Minted before any artifact exists, because the jail root is a function of
    // the VM id and the artifacts are built inside it.
    let vm_id = VmId::new();
    // Shared by this job's cgroup and chroot: a cgroup that outlives its
    // teardown keeps the chroot that names it.
    let cgroup_survived = CgroupSurvived::default();
    let jail_dir = JailDir::create(log, &state_dir, &vm_id, cgroup_survived.clone())?;
    let jail = JailPaths::new(jail_dir.root())?;
    info!(log, "Jail built"; "jail_root" => jail.root().as_str());

    // Everything Firecracker reads has to be inside the chroot, so the kernel
    // lands in the jail root whatever its source.
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
    bencher_rootfs::create_ext4_with_size(unpack_dir, rootfs_dest, config.disk.to_mib())?;

    // The jailer does not chown what the runner placed in the chroot: the
    // rootfs is written by Firecracker so it is given away, while the kernel is
    // only read so it stays root-owned.
    chroot::chown_to_jail(rootfs_dest, config.jail_user)?;
    chroot::grant_jail_read(kernel_dest)?;

    // Step 7-8: Build Firecracker config and run the microVM
    let fc_config = build_firecracker_config(
        log,
        config,
        work_dir,
        vm_id,
        &state_dir,
        jail,
        netns,
        cgroup_survived,
    )?;

    refuse_cancelled(cancel_flag)?;
    let run_output = run_firecracker(log, &fc_config, cancel_flag, deadline)?;

    Ok(run_output)
}

/// Build the Firecracker job config: stage the binaries and convert types.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one part of the VM's config"
)]
fn build_firecracker_config(
    log: &Logger,
    config: &crate::Config,
    work_dir: &Utf8Path,
    vm_id: VmId,
    state_dir: &StateDir,
    jail: JailPaths,
    netns: Utf8PathBuf,
    cgroup_survived: CgroupSurvived,
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

    Ok(crate::firecracker::FirecrackerJobConfig {
        firecracker_bin,
        jailer_bin,
        vm_id,
        jail,
        jail_user: config.jail_user,
        chroot_base_dir: state_dir.chroot_base(),
        netns,
        cgroup_survived,
        vcpus,
        memory_mib,
        boot_args: config.kernel_cmdline.clone(),
        cpu_layout: config.cpu_layout.clone(),
        log_level: config.sandbox_log_level,
        max_file_count: config.max_file_count,
        max_content_size: config.max_content_size,
        max_output_size: config.max_output_size,
        grace_period: config.grace_period,
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
