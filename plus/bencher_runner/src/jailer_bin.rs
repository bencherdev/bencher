//! The bundled Firecracker `jailer`: embedded in release builds, and loaded
//! from the build script's download in debug builds.

use std::io;

use camino::Utf8Path;

include!(concat!(env!("OUT_DIR"), "/jailer_generated.rs"));

pub fn write_jailer_to_file(path: &Utf8Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::write(path, jailer_bytes())?;

    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;

    Ok(())
}
