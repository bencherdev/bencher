//! The drive a guest leaves its results on, made before the VMM starts and read
//! only once every jailed process is gone.

use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt as _;

use bencher_output_protocol::results;
use camino::{Utf8Path, Utf8PathBuf};

use crate::firecracker::error::FirecrackerError;
use crate::jail::chroot::chown_to_jail;
use crate::jail::{CgroupManager, JailUser};
use crate::run::RunOutput;

/// Held open read-only from before any guest code ran, so the host reads the
/// file it made whatever the jail later does to its name.
#[derive(Debug)]
pub struct ResultsDrive {
    file: File,
    cap: u64,
}

impl ResultsDrive {
    /// A sparse file sized for a record whose stdout, stderr, and output files
    /// are each at `max_output_size`, given to the jail user like the rootfs.
    pub fn create(
        path: &Utf8Path,
        jail_user: JailUser,
        max_output_size: usize,
    ) -> Result<Self, FirecrackerError> {
        let (cap, size) = u64::try_from(max_output_size)
            .ok()
            .and_then(|cap| Some((cap, results::drive_size(cap)?)))
            .ok_or(FirecrackerError::ResultsDriveTooLarge { max_output_size })?;
        let failed = |source| FirecrackerError::CreateResultsDrive {
            path: path.to_owned(),
            source,
        };
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(failed)?
            .set_len(size)
            .map_err(failed)?;
        chown_to_jail(path, jail_user).map_err(FirecrackerError::Chown)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(failed)?;
        Ok(Self { file, cap })
    }

    /// Empties the VM's cgroup first, since a process left in the jail could
    /// still write the drive.
    pub fn read_from_emptied_jail(
        &self,
        cgroup: &CgroupManager,
        max_file_count: u32,
        max_content_size: u64,
    ) -> Result<RunOutput, FirecrackerError> {
        cgroup.empty().map_err(FirecrackerError::JailNotEmptied)?;
        self.read(max_file_count, max_content_size)
    }

    fn read(
        &self,
        max_file_count: u32,
        max_content_size: u64,
    ) -> Result<RunOutput, FirecrackerError> {
        let record = results::read(&self.file, self.cap)
            .map_err(FirecrackerError::ReadResults)?
            .ok_or(FirecrackerError::NoResults)?;
        let output_files = if record.output_files.is_empty() {
            None
        } else {
            Some(decode_output_files(
                &record.output_files,
                max_file_count,
                max_content_size,
            )?)
        };
        Ok(RunOutput {
            exit_code: record.exit_code,
            stdout: String::from_utf8_lossy(&record.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&record.stderr).into_owned(),
            output_files,
        })
    }
}

fn decode_output_files(
    data: &[u8],
    max_file_count: u32,
    max_content_size: u64,
) -> Result<Vec<(Utf8PathBuf, Vec<u8>)>, FirecrackerError> {
    bencher_output_protocol::decode(data, max_file_count, max_content_size)
        .map_err(|source| FirecrackerError::DecodeOutputFiles { source })
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};

    use bencher_output_protocol::results::Record;

    use super::*;

    const CAP: usize = 64 * 1024;

    /// The jail user, or the test's own user where it cannot give a file away.
    fn jail_user() -> JailUser {
        let uid = crate::jail::current_euid();
        if uid == 0 {
            JailUser::default()
        } else {
            #[expect(unsafe_code, reason = "getegid has no std wrapper")]
            // SAFETY: `getegid` takes nothing and cannot fail.
            let gid = unsafe { libc::getegid() };
            JailUser::new(uid, gid).unwrap()
        }
    }

    fn drive_in_tmpdir() -> (tempfile::TempDir, Utf8PathBuf, ResultsDrive) {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap().join("results.img");
        let drive = ResultsDrive::create(&path, jail_user(), CAP).unwrap();
        (dir, path, drive)
    }

    /// Writes as the guest does, through a handle of its own.
    fn guest_writes(path: &Utf8Path, record: &Record) {
        let mut guest = OpenOptions::new().write(true).open(path).unwrap();
        results::write(&mut guest, record).unwrap();
    }

    fn files(paths: &[&str]) -> Vec<u8> {
        let files: Vec<(&Utf8Path, &[u8])> = paths
            .iter()
            .map(|path| (Utf8Path::new(path), path.as_bytes()))
            .collect();
        bencher_output_protocol::encode(&files).unwrap()
    }

    #[test]
    fn a_guest_that_wrote_nothing_stopped_without_results() {
        // Kills reading a drive with no record as a run of exit code 0 and no
        // output, which reports a guest that died early as a success.
        let (_dir, _path, drive) = drive_in_tmpdir();

        let err = drive.read(255, 1024).unwrap_err();

        assert!(matches!(err, FirecrackerError::NoResults), "{err}");
    }

    #[test]
    fn the_record_the_guest_wrote_is_the_run_output() {
        // Kills stdout and stderr swapped on the way out, and output files
        // dropped rather than decoded.
        let (_dir, path, drive) = drive_in_tmpdir();
        guest_writes(
            &path,
            &Record {
                exit_code: 42,
                stdout: b"out".to_vec(),
                stderr: b"err".to_vec(),
                output_files: files(&["/b.json", "/a.json"]),
            },
        );

        let output = drive.read(255, 1024).unwrap();

        assert_eq!(output.exit_code, 42);
        assert_eq!(output.stdout, "out");
        assert_eq!(output.stderr, "err");
        assert_eq!(
            output.output_files,
            Some(vec![
                (Utf8PathBuf::from("/b.json"), b"/b.json".to_vec()),
                (Utf8PathBuf::from("/a.json"), b"/a.json".to_vec()),
            ])
        );
    }

    #[test]
    fn output_files_decode_in_the_order_the_guest_sent_them() {
        // Descending, and enough files that neither a sorted nor a hashed order matches.
        let paths: Vec<String> = (0..32).rev().map(|n| format!("/{n}.out")).collect();
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let (_dir, path, drive) = drive_in_tmpdir();
        guest_writes(
            &path,
            &Record {
                output_files: files(&paths),
                ..Record::default()
            },
        );

        let output = drive.read(32, 1024).unwrap();

        let decoded: Vec<Utf8PathBuf> = output
            .output_files
            .unwrap()
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        assert_eq!(decoded, paths);
    }

    #[test]
    fn output_files_past_the_file_count_cap_fail_the_run() {
        // Kills decoding the guest's files with no cap, or a cap other than the
        // Job's.
        let (_dir, path, drive) = drive_in_tmpdir();
        guest_writes(
            &path,
            &Record {
                output_files: files(&["/a", "/b", "/c"]),
                ..Record::default()
            },
        );

        let err = drive.read(2, 1024).unwrap_err();

        assert!(
            matches!(err, FirecrackerError::DecodeOutputFiles { .. }),
            "{err}"
        );
    }

    #[test]
    fn a_drive_whose_name_was_replaced_is_still_the_one_made() {
        // Kills a read that reopens the drive by name, which a compromised VMM
        // can point at a record of its choosing.
        let (dir, path, drive) = drive_in_tmpdir();
        let forged = Utf8Path::from_path(dir.path()).unwrap().join("forged.img");
        std::fs::copy(&path, &forged).unwrap();
        guest_writes(
            &forged,
            &Record {
                exit_code: 0,
                stdout: b"forged".to_vec(),
                ..Record::default()
            },
        );
        std::fs::rename(&forged, &path).unwrap();

        let err = drive.read(255, 1024).unwrap_err();

        assert!(matches!(err, FirecrackerError::NoResults), "{err}");
    }

    #[test]
    fn the_descriptor_kept_cannot_write_the_drive() {
        // Kills a drive kept open for writing, which lets the host change what
        // the guest left.
        let (_dir, _path, drive) = drive_in_tmpdir();

        (&drive.file).write_all(b"x").unwrap_err();
    }

    #[test]
    fn the_drive_is_sparse_private_and_the_jail_users() {
        // Kills a drive the VMM cannot open as the jail user, one other users
        // can, and one that takes its full size on disk.
        let (_dir, path, _drive) = drive_in_tmpdir();

        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(
            metadata.len(),
            results::drive_size(CAP as u64).unwrap(),
            "sized for the caps"
        );
        assert_eq!(metadata.uid(), jail_user().uid());
        assert_eq!(metadata.gid(), jail_user().gid());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            metadata.blocks(),
            0,
            "nothing is written until the guest writes"
        );
    }

    /// A stand-in for a VM cgroup whose `cgroup.events` reports `populated`.
    fn cgroup(populated: bool) -> (tempfile::TempDir, CgroupManager) {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        std::fs::write(path.join("cgroup.kill"), "").unwrap();
        std::fs::write(
            path.join("cgroup.events"),
            format!("populated {}\nfrozen 0\n", u8::from(populated)),
        )
        .unwrap();
        (dir, CgroupManager::detached(path))
    }

    #[test]
    fn a_jail_that_will_not_empty_is_never_read() {
        // Kills a read before the jail is empty, which takes a record a process
        // left in the jail can still rewrite.
        let (_dir, path, drive) = drive_in_tmpdir();
        guest_writes(
            &path,
            &Record {
                stdout: b"out".to_vec(),
                ..Record::default()
            },
        );
        let (_cgroup_dir, occupied) = cgroup(true);

        let err = drive
            .read_from_emptied_jail(&occupied, 255, 1024)
            .unwrap_err();

        assert!(matches!(err, FirecrackerError::JailNotEmptied(_)), "{err}");
    }

    #[test]
    fn an_emptied_jail_is_killed_then_read() {
        // Kills a read that skips the kill, which leaves a process the VMM
        // started free to write the drive.
        let (_dir, path, drive) = drive_in_tmpdir();
        guest_writes(
            &path,
            &Record {
                stdout: b"out".to_vec(),
                ..Record::default()
            },
        );
        let (cgroup_dir, empty) = cgroup(false);

        let output = drive.read_from_emptied_jail(&empty, 255, 1024).unwrap();

        assert_eq!(output.stdout, "out");
        assert_eq!(
            std::fs::read_to_string(cgroup_dir.path().join("cgroup.kill")).unwrap(),
            "1",
            "the cgroup is killed before the read"
        );
    }

    #[test]
    fn a_path_already_taken_is_refused_not_reused() {
        // Kills a create that reuses or truncates what is already at the path,
        // a file or a link to one.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let elsewhere = root.join("elsewhere");
        std::fs::write(&elsewhere, b"not the jail's").unwrap();
        let planted = root.join("planted.img");
        std::fs::write(&planted, b"planted").unwrap();
        let linked = root.join("linked.img");
        symlink(&elsewhere, &linked).unwrap();

        for path in [&planted, &linked] {
            let err = ResultsDrive::create(path, jail_user(), CAP).unwrap_err();
            assert!(
                matches!(
                    &err,
                    FirecrackerError::CreateResultsDrive { source, .. }
                        if source.kind() == std::io::ErrorKind::AlreadyExists
                ),
                "{path}: {err}"
            );
        }
        assert_eq!(std::fs::read(&planted).unwrap(), b"planted");
        assert_eq!(std::fs::read(&elsewhere).unwrap(), b"not the jail's");
    }

    #[test]
    fn a_cap_no_drive_can_hold_is_refused_before_a_file_exists() {
        // Kills a drive size that wraps to a small file.
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap().join("results.img");

        let err = ResultsDrive::create(&path, jail_user(), usize::MAX).unwrap_err();

        assert!(
            matches!(err, FirecrackerError::ResultsDriveTooLarge { .. }),
            "{err}"
        );
        assert!(!path.exists(), "no drive is made");
    }
}
