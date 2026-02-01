//! Chroot and pivot_root management.
//!
//! This module handles setting up an isolated filesystem root for the jailed process.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::unistd::{chdir, pivot_root};

use crate::config::BindMount;
use crate::error::JailerError;

/// Set up the jail root filesystem.
///
/// This creates the minimal directory structure needed for the jail:
/// - /proc (mounted)
/// - /dev (minimal, with null, zero, urandom)
/// - /tmp
/// - Any bind mounts specified in config
pub fn setup_jail_root(
    jail_root: &Path,
    bind_mounts: &[BindMount],
) -> Result<(), JailerError> {
    // Create jail root if it doesn't exist
    fs::create_dir_all(jail_root)
        .map_err(|e| JailerError::Chroot(format!("failed to create jail root: {e}")))?;

    // Create essential directories
    create_dir(jail_root, "proc")?;
    create_dir(jail_root, "dev")?;
    create_dir(jail_root, "tmp")?;
    create_dir(jail_root, "etc")?;
    create_dir(jail_root, "run")?;

    // Set up bind mounts
    for bind in bind_mounts {
        setup_bind_mount(jail_root, bind)?;
    }

    Ok(())
}

/// Create a directory inside the jail root.
fn create_dir(jail_root: &Path, subpath: &str) -> Result<(), JailerError> {
    let path = jail_root.join(subpath);
    if !path.exists() {
        fs::create_dir_all(&path)
            .map_err(|e| JailerError::Chroot(format!("failed to create {}: {e}", path.display())))?;
    }
    Ok(())
}

/// Set up a bind mount.
fn setup_bind_mount(jail_root: &Path, bind: &BindMount) -> Result<(), JailerError> {
    let dest = jail_root.join(bind.dest.as_str().trim_start_matches('/'));

    // Create destination directory
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            JailerError::Chroot(format!("failed to create mount point parent: {e}"))
        })?;
    }

    // Create mount point (file or directory depending on source)
    let source_path = Path::new(bind.source.as_str());
    if source_path.is_file() {
        if !dest.exists() {
            fs::write(&dest, "").map_err(|e| {
                JailerError::Chroot(format!("failed to create mount point file: {e}"))
            })?;
        }
    } else {
        fs::create_dir_all(&dest).map_err(|e| {
            JailerError::Chroot(format!("failed to create mount point dir: {e}"))
        })?;
    }

    // Perform bind mount
    let mut flags = MsFlags::MS_BIND;
    if bind.read_only {
        flags |= MsFlags::MS_RDONLY;
    }

    mount(
        Some(source_path),
        &dest,
        None::<&str>,
        flags,
        None::<&str>,
    )
    .map_err(|e| JailerError::Chroot(format!("bind mount failed: {e}")))?;

    // Remount read-only if requested (bind mounts need a remount to apply ro)
    if bind.read_only {
        mount(
            None::<&str>,
            &dest,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            None::<&str>,
        )
        .map_err(|e| JailerError::Chroot(format!("bind remount ro failed: {e}")))?;
    }

    Ok(())
}

/// Mount essential filesystems inside the jail.
///
/// This should be called after pivot_root.
pub fn mount_essential_filesystems(jail_root: &Path) -> Result<(), JailerError> {
    // Mount /proc
    let proc_path = jail_root.join("proc");
    mount(
        Some("proc"),
        &proc_path,
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        None::<&str>,
    )
    .map_err(|e| JailerError::Chroot(format!("failed to mount /proc: {e}")))?;

    // Set up minimal /dev
    setup_minimal_dev(jail_root)?;

    Ok(())
}

/// Set up a minimal /dev with only essential devices.
fn setup_minimal_dev(jail_root: &Path) -> Result<(), JailerError> {
    let dev_path = jail_root.join("dev");

    // Mount a tmpfs for /dev
    mount(
        Some("tmpfs"),
        &dev_path,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
        Some("mode=755,size=65536"),
    )
    .map_err(|e| JailerError::Chroot(format!("failed to mount /dev tmpfs: {e}")))?;

    // Create essential device nodes by bind mounting from host
    // This is safer than mknod as it doesn't require CAP_MKNOD
    create_dev_node(&dev_path, "null")?;
    create_dev_node(&dev_path, "zero")?;
    create_dev_node(&dev_path, "urandom")?;
    create_dev_node(&dev_path, "random")?;

    // Create /dev/kvm if it exists (needed for VMM)
    if Path::new("/dev/kvm").exists() {
        create_dev_node(&dev_path, "kvm")?;
    }

    Ok(())
}

/// Create a device node by bind mounting from host.
fn create_dev_node(dev_path: &Path, name: &str) -> Result<(), JailerError> {
    let source = Path::new("/dev").join(name);
    let dest = dev_path.join(name);

    if !source.exists() {
        // Device doesn't exist on host, skip
        return Ok(());
    }

    // Create empty file as mount point
    fs::write(&dest, "")
        .map_err(|e| JailerError::Chroot(format!("failed to create /dev/{name}: {e}")))?;

    // Bind mount the device
    mount(
        Some(&source),
        &dest,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )
    .map_err(|e| JailerError::Chroot(format!("failed to bind mount /dev/{name}: {e}")))?;

    Ok(())
}

/// Perform pivot_root to change the root filesystem.
///
/// This is more secure than chroot as it also changes the mount namespace root.
pub fn do_pivot_root(new_root: &Path) -> Result<(), JailerError> {
    // First, bind mount new_root to itself (required for pivot_root)
    mount(
        Some(new_root),
        new_root,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| JailerError::Chroot(format!("bind mount new_root failed: {e}")))?;

    // Create old_root inside new_root
    let old_root = new_root.join(".old_root");
    fs::create_dir_all(&old_root)
        .map_err(|e| JailerError::Chroot(format!("failed to create old_root: {e}")))?;

    // Change to new root
    chdir(new_root).map_err(|e| JailerError::Chroot(format!("chdir to new_root failed: {e}")))?;

    // Pivot root
    pivot_root(".", ".old_root")
        .map_err(|e| JailerError::Chroot(format!("pivot_root failed: {e}")))?;

    // Change to / in new root
    chdir("/").map_err(|e| JailerError::Chroot(format!("chdir to / failed: {e}")))?;

    // Unmount old root
    umount2("/.old_root", MntFlags::MNT_DETACH)
        .map_err(|e| JailerError::Chroot(format!("unmount old_root failed: {e}")))?;

    // Remove old_root directory
    fs::remove_dir("/.old_root")
        .map_err(|e| JailerError::Chroot(format!("failed to remove old_root: {e}")))?;

    Ok(())
}
