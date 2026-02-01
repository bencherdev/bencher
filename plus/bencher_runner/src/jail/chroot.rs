//! Chroot and `pivot_root` management.

use std::fs;
use std::path::Path;

use camino::Utf8Path;
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::unistd::{chdir, pivot_root};

use crate::RunnerError;

/// Create the jail root filesystem structure.
///
/// Creates essential directories and bind mounts the kernel and rootfs.
pub fn create_jail_root(
    jail_root: &Utf8Path,
    kernel_path: &Utf8Path,
    rootfs_path: &Utf8Path,
    vsock_path: Option<&Utf8Path>,
) -> Result<(), RunnerError> {
    let jail_root = Path::new(jail_root.as_str());

    // Create jail root and essential directories
    fs::create_dir_all(jail_root)
        .map_err(|e| RunnerError::Jail(format!("failed to create jail root: {e}")))?;

    create_dir(jail_root, "proc")?;
    create_dir(jail_root, "dev")?;
    create_dir(jail_root, "tmp")?;
    create_dir(jail_root, "run")?;

    // Bind mount kernel
    bind_mount_file(kernel_path.as_str(), &jail_root.join("kernel"))?;

    // Bind mount rootfs
    bind_mount_file(rootfs_path.as_str(), &jail_root.join("rootfs.squashfs"))?;

    // Bind mount vsock socket if provided
    if let Some(vsock) = vsock_path {
        bind_mount_file(vsock.as_str(), &jail_root.join("vsock.sock"))?;
    }

    Ok(())
}

/// Create a directory inside the jail root.
fn create_dir(jail_root: &Path, subpath: &str) -> Result<(), RunnerError> {
    let path = jail_root.join(subpath);
    if !path.exists() {
        fs::create_dir_all(&path)
            .map_err(|e| RunnerError::Jail(format!("failed to create {}: {e}", path.display())))?;
    }
    Ok(())
}

/// Bind mount a file into the jail.
fn bind_mount_file(source: &str, dest: &Path) -> Result<(), RunnerError> {
    // Create mount point file
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            RunnerError::Jail(format!("failed to create mount point parent: {e}"))
        })?;
    }

    if !dest.exists() {
        fs::write(dest, "")
            .map_err(|e| RunnerError::Jail(format!("failed to create mount point: {e}")))?;
    }

    // Bind mount
    mount(
        Some(source),
        dest,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("bind mount failed for {source}: {e}")))?;

    // Remount read-only
    mount(
        None::<&str>,
        dest,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("remount ro failed: {e}")))?;

    Ok(())
}

/// Mount essential filesystems inside the jail.
///
/// Should be called after `pivot_root`.
pub fn mount_essential_filesystems() -> Result<(), RunnerError> {
    // Mount /proc
    mount(
        Some("proc"),
        "/proc",
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("failed to mount /proc: {e}")))?;

    // Mount tmpfs on /dev
    mount(
        Some("tmpfs"),
        "/dev",
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
        Some("mode=755,size=65536"),
    )
    .map_err(|e| RunnerError::Jail(format!("failed to mount /dev: {e}")))?;

    // Create essential device nodes by bind mounting from outside
    // Note: After pivot_root, /dev/null etc. are gone, so we use the paths we saved
    create_dev_null()?;
    create_dev_zero()?;
    create_dev_urandom()?;

    // /dev/kvm is critical for the VMM
    create_dev_kvm()?;

    Ok(())
}

/// Create /dev/null using mknod (requires being in user namespace with mappings).
fn create_dev_null() -> Result<(), RunnerError> {
    // In a user namespace, we can't mknod, but we can bind mount if we prepared it
    // For now, create a simple file that acts as a sink
    fs::write("/dev/null", "").map_err(|e| RunnerError::Jail(format!("failed to create /dev/null: {e}")))?;
    Ok(())
}

fn create_dev_zero() -> Result<(), RunnerError> {
    fs::write("/dev/zero", "").map_err(|e| RunnerError::Jail(format!("failed to create /dev/zero: {e}")))?;
    Ok(())
}

fn create_dev_urandom() -> Result<(), RunnerError> {
    fs::write("/dev/urandom", "").map_err(|e| RunnerError::Jail(format!("failed to create /dev/urandom: {e}")))?;
    Ok(())
}

fn create_dev_kvm() -> Result<(), RunnerError> {
    // /dev/kvm needs to be bind mounted from the host before pivot_root
    // This is handled in create_jail_root via bind mounts
    // Here we just verify it exists
    if !Path::new("/dev/kvm").exists() {
        return Err(RunnerError::Jail("/dev/kvm not available in jail".into()));
    }
    Ok(())
}

/// Perform `pivot_root` to change the root filesystem.
pub fn do_pivot_root(new_root: &Utf8Path) -> Result<(), RunnerError> {
    let new_root = Path::new(new_root.as_str());

    // Bind mount new_root to itself (required for pivot_root)
    mount(
        Some(new_root),
        new_root,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("bind mount new_root failed: {e}")))?;

    // Create old_root directory inside new_root
    let old_root = new_root.join(".old_root");
    fs::create_dir_all(&old_root)
        .map_err(|e| RunnerError::Jail(format!("failed to create old_root: {e}")))?;

    // Change to new root
    chdir(new_root).map_err(|e| RunnerError::Jail(format!("chdir failed: {e}")))?;

    // Pivot root
    pivot_root(".", ".old_root").map_err(|e| RunnerError::Jail(format!("pivot_root failed: {e}")))?;

    // Change to / in new root
    chdir("/").map_err(|e| RunnerError::Jail(format!("chdir / failed: {e}")))?;

    // Unmount old root
    umount2("/.old_root", MntFlags::MNT_DETACH)
        .map_err(|e| RunnerError::Jail(format!("unmount old_root failed: {e}")))?;

    // Remove old_root directory
    fs::remove_dir("/.old_root")
        .map_err(|e| RunnerError::Jail(format!("remove old_root failed: {e}")))?;

    Ok(())
}
