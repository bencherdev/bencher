use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::process::Command;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};

use crate::parser::TaskOci;

/// Map `std::env::consts::ARCH` to OCI platform architecture names.
fn current_oci_arch() -> anyhow::Result<&'static str> {
    use std::env::consts::ARCH;
    match ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        arch => anyhow::bail!("Unsupported architecture: {arch}"),
    }
}

/// Get the busybox download URL for the current architecture.
fn busybox_url() -> anyhow::Result<&'static str> {
    use std::env::consts::ARCH;
    match ARCH {
        "x86_64" => Ok("https://busybox.net/downloads/binaries/1.35.0-x86_64-linux-musl/busybox"),
        "aarch64" => Ok("https://busybox.net/downloads/binaries/1.35.0-aarch64-linux-musl/busybox"),
        arch => anyhow::bail!("Unsupported architecture: {arch}"),
    }
}

/// Get the expected SHA256 hash of the busybox binary for the current architecture.
fn busybox_sha256() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "6e123e7f3202a8c1e9b1f94d8941580a25135382b99e8d3e34fb858bba311348",
        "aarch64" => "65643147a622ddf4689ade5f66a21f7cafb378ec595b4da3cd78c21412bf2230",
        arch => panic!("Unsupported architecture: {arch}"),
    }
}

#[derive(Debug)]
pub struct Oci {}

impl TryFrom<TaskOci> for Oci {
    type Error = anyhow::Error;

    fn try_from(_oci: TaskOci) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Oci {
    #[expect(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        create_test_image()
    }
}

/// Get the path to the OCI image directory.
pub fn oci_image_path() -> Utf8PathBuf {
    super::work_dir().join("oci-image")
}

/// Get the path to the rootfs directory (unpacked).
fn rootfs_path() -> Utf8PathBuf {
    super::work_dir().join("rootfs")
}

/// Create the test OCI image.
pub fn create_test_image() -> anyhow::Result<()> {
    let oci_path = oci_image_path();

    if oci_path.exists() {
        println!("OCI image already exists at {oci_path}");
        return Ok(());
    }

    println!("Creating test OCI image...");

    // First, create the rootfs
    let rootfs = rootfs_path();
    create_rootfs(&rootfs)?;

    // Then package it as OCI
    package_as_oci(&rootfs, &oci_path)?;

    println!("OCI image created at {oci_path}");
    Ok(())
}

/// Create a minimal rootfs with busybox and bencher.
fn create_rootfs(rootfs: &Utf8Path) -> anyhow::Result<()> {
    println!("Creating rootfs at {rootfs}...");

    // Clean and create directories
    if rootfs.exists() {
        fs::remove_dir_all(rootfs)?;
    }

    let dirs = [
        "bin", "sbin", "usr/bin", "usr/sbin", "etc", "dev", "proc", "sys", "tmp", "root",
    ];
    for dir in dirs {
        fs::create_dir_all(rootfs.join(dir))?;
    }

    // Download and install busybox
    install_busybox(rootfs)?;

    // Install bencher CLI
    install_bencher(rootfs)?;

    // Create init script
    create_init_script(rootfs)?;

    // Write the command config (this would normally be done by the runner
    // after parsing the OCI config, but for the test image we hardcode it)
    write_command_config(rootfs, &["/usr/bin/bencher", "mock"], "/", &[])?;

    // Create basic /etc files
    create_etc_files(rootfs)?;

    println!("Rootfs created successfully");
    Ok(())
}

/// Download and install busybox.
fn install_busybox(rootfs: &Utf8Path) -> anyhow::Result<()> {
    let busybox_url = busybox_url()?;
    let busybox_path = rootfs.join("bin/busybox");

    println!("Downloading busybox...");
    let response = reqwest::blocking::get(busybox_url).context("Failed to download busybox")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download busybox: HTTP {}", response.status());
    }

    let bytes = response.bytes()?;

    // Verify SHA256 checksum
    let hash = sha256_hex(&bytes);
    let expected = busybox_sha256();
    anyhow::ensure!(
        hash == expected,
        "Busybox checksum mismatch: expected {expected}, got {hash}"
    );

    let mut file = File::create(&busybox_path)?;
    file.write_all(&bytes)?;

    // Make executable
    fs::set_permissions(&busybox_path, fs::Permissions::from_mode(0o755))?;

    // Create symlinks for common utilities
    let utils = [
        "sh", "ash", "cat", "echo", "ls", "mkdir", "mount", "umount", "sleep", "ps", "kill", "pwd",
        "rm", "cp", "mv", "ln", "grep", "sed", "awk", "head", "tail", "wc", "tr", "env",
        "printenv", "hostname", "uname", "id", "whoami", "chmod", "chown", "date", "true", "false",
        "test", "poweroff", "reboot", "halt", "init",
    ];

    for util in utils {
        let link_path = rootfs.join(format!("bin/{util}"));
        if !link_path.exists() {
            symlink("busybox", &link_path)?;
        }
    }

    // Also create /sbin links for init utilities
    for util in ["init", "poweroff", "reboot", "halt"] {
        let link_path = rootfs.join(format!("sbin/{util}"));
        if !link_path.exists() {
            symlink("../bin/busybox", &link_path)?;
        }
    }

    println!("Busybox installed");
    Ok(())
}

/// Install the bencher CLI.
///
/// Tries to build a statically linked (musl) binary first, then falls back
/// to the default target (glibc), and finally to a mock shell script.
/// The binary must be statically linked to run inside the minimal busybox rootfs.
fn install_bencher(rootfs: &Utf8Path) -> anyhow::Result<()> {
    let workspace_root = super::workspace_root();

    let bencher_dst = rootfs.join("usr/bin/bencher");

    // Try musl (statically linked) first — required for minimal rootfs
    let target_triple = super::musl_target_triple()?;
    println!("Building bencher CLI (musl, {target_triple})...");
    let musl_status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            target_triple,
            "-p",
            "bencher_cli",
        ])
        .current_dir(&workspace_root)
        .status();

    if let Ok(status) = musl_status
        && status.success()
    {
        let musl_src = workspace_root.join(format!("target/{target_triple}/release/bencher"));
        if musl_src.exists() {
            fs::copy(&musl_src, &bencher_dst)?;
            fs::set_permissions(&bencher_dst, fs::Permissions::from_mode(0o755))?;
            println!("Bencher CLI installed (statically linked)");
            return Ok(());
        }
    }

    // Musl build failed — fall back to mock script
    // (a dynamically linked binary won't work in the minimal busybox rootfs)
    println!("Warning: musl build failed, using mock bencher script");
    create_mock_bencher(rootfs)?;

    Ok(())
}

/// Create a mock bencher script for testing.
fn create_mock_bencher(rootfs: &Utf8Path) -> anyhow::Result<()> {
    // Matches the format of `bencher mock` with 5 results
    let script = r#"#!/bin/sh
# Mock bencher script for testing
case "$1" in
    mock)
        cat << 'EOF'
{
  "bencher::mock_0": {
    "latency": {
      "value": 4.5535649932187034,
      "lower_value": 4.098208493896833,
      "upper_value": 5.008921492540574
    }
  },
  "bencher::mock_1": {
    "latency": {
      "value": 16.537506086518523,
      "lower_value": 14.88375547786667,
      "upper_value": 18.191256695170374
    }
  },
  "bencher::mock_2": {
    "latency": {
      "value": 20.221420814607537,
      "lower_value": 18.199278733146784,
      "upper_value": 22.24356289606829
    }
  },
  "bencher::mock_3": {
    "latency": {
      "value": 34.92859461603261,
      "lower_value": 31.435735154429352,
      "upper_value": 38.42145407763587
    }
  },
  "bencher::mock_4": {
    "latency": {
      "value": 42.40432493036204,
      "lower_value": 38.163892437325835,
      "upper_value": 46.64475742339824
    }
  }
}
EOF
        ;;
    *)
        echo "Usage: bencher mock"
        exit 1
        ;;
esac
"#;

    let path = rootfs.join("usr/bin/bencher");
    let mut file = File::create(&path)?;
    file.write_all(script.as_bytes())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// Create the init script that reads command from OCI config.
fn create_init_script(rootfs: &Utf8Path) -> anyhow::Result<()> {
    // Create the bencher config directory
    fs::create_dir_all(rootfs.join("etc/bencher"))?;

    let init_script = r#"#!/bin/sh
# Bencher runner init script
# This script reads the command from /etc/bencher/command and executes it

# Mount essential filesystems
mount -t proc none /proc 2>/dev/null
mount -t sysfs none /sys 2>/dev/null
mount -t devtmpfs none /dev 2>/dev/null

# Print startup message
echo "=== Bencher Runner VM ==="

# Read and execute the command from config
if [ -f /etc/bencher/command ]; then
    echo "Executing command from OCI config..."
    # Source environment if present
    if [ -f /etc/bencher/env ]; then
        . /etc/bencher/env
    fi
    # Change to working directory if specified
    if [ -f /etc/bencher/workdir ]; then
        cd "$(cat /etc/bencher/workdir)" || true
    fi
    # Execute the command
    echo ""
    sh -c "$(cat /etc/bencher/command)"
    EXIT_CODE=$?
    echo ""
    echo "=== Command exited with code: $EXIT_CODE ==="
else
    echo "ERROR: No command found at /etc/bencher/command"
    echo "The OCI image must have CMD or ENTRYPOINT set"
fi

# Shutdown
echo "Shutting down..."
poweroff -f
"#;

    let path = rootfs.join("init");
    let mut file = File::create(&path)?;
    file.write_all(init_script.as_bytes())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;

    println!("Init script created");
    Ok(())
}

/// Write the command configuration files for the init script.
///
/// This writes:
/// - `/etc/bencher/command` - The shell command to execute
/// - `/etc/bencher/workdir` - The working directory (optional)
/// - `/etc/bencher/env` - Environment variables as shell exports (optional)
fn write_command_config(
    rootfs: &Utf8Path,
    command: &[&str],
    workdir: &str,
    env: &[(String, String)],
) -> anyhow::Result<()> {
    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Write the command as a shell-escaped string
    let command_str = command
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    fs::write(config_dir.join("command"), command_str)?;

    // Write working directory
    if !workdir.is_empty() && workdir != "/" {
        fs::write(config_dir.join("workdir"), workdir)?;
    }

    // Write environment as shell exports
    if !env.is_empty() {
        use std::fmt::Write as _;
        let mut env_script = String::new();
        for (k, v) in env {
            writeln!(env_script, "export {}={}", k, shell_escape(v))
                .expect("writing to String cannot fail");
        }
        fs::write(config_dir.join("env"), env_script)?;
    }

    Ok(())
}

/// Simple shell escaping for arguments.
fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.')
    {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

/// Create basic /etc files.
fn create_etc_files(rootfs: &Utf8Path) -> anyhow::Result<()> {
    // /etc/passwd
    let passwd = "root:x:0:0:root:/root:/bin/sh\n";
    fs::write(rootfs.join("etc/passwd"), passwd)?;

    // /etc/group
    let group = "root:x:0:\n";
    fs::write(rootfs.join("etc/group"), group)?;

    // /etc/hostname
    fs::write(rootfs.join("etc/hostname"), "bencher-vm\n")?;

    Ok(())
}

/// Package a rootfs directory as an OCI image.
fn package_as_oci(rootfs: &Utf8Path, oci_path: &Utf8Path) -> anyhow::Result<()> {
    println!("Packaging rootfs as OCI image...");

    // Create OCI directory structure
    fs::create_dir_all(oci_path.join("blobs/sha256"))?;

    // Create the layer tarball
    let layer_tar = super::work_dir().join("layer.tar.gz");
    let diff_id = create_layer_tarball(rootfs, &layer_tar)?;

    // Calculate layer digest
    let layer_bytes = fs::read(&layer_tar)?;
    let layer_digest = sha256_hex(&layer_bytes);
    let layer_size = layer_bytes.len();

    // Move layer to blobs
    let layer_blob_path = oci_path.join(format!("blobs/sha256/{layer_digest}"));
    fs::rename(&layer_tar, &layer_blob_path)?;

    // Resolve architecture once for config and index
    let oci_arch = current_oci_arch()?;

    // Create config
    let config = create_image_config(&diff_id, oci_arch);
    let config_bytes = serde_json::to_vec(&config)?;
    let config_digest = sha256_hex(&config_bytes);
    let config_size = config_bytes.len();
    fs::write(
        oci_path.join(format!("blobs/sha256/{config_digest}")),
        &config_bytes,
    )?;

    // Create manifest
    let manifest = create_manifest(&layer_digest, layer_size, &config_digest, config_size);
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    let manifest_digest = sha256_hex(&manifest_bytes);
    fs::write(
        oci_path.join(format!("blobs/sha256/{manifest_digest}")),
        &manifest_bytes,
    )?;

    // Create index.json
    let index = create_index(&manifest_digest, manifest_bytes.len(), oci_arch);
    fs::write(
        oci_path.join("index.json"),
        serde_json::to_vec_pretty(&index)?,
    )?;

    // Create oci-layout
    let layout = serde_json::json!({
        "imageLayoutVersion": "1.0.0"
    });
    fs::write(
        oci_path.join("oci-layout"),
        serde_json::to_vec_pretty(&layout)?,
    )?;

    // Clean up temp files
    drop(fs::remove_file(super::work_dir().join("layer.tar")));

    println!("OCI image packaged successfully");
    Ok(())
}

/// Create a gzipped tarball of the rootfs.
/// Returns the SHA256 digest of the uncompressed tar (for `diff_ids`).
fn create_layer_tarball(rootfs: &Utf8Path, output: &Utf8Path) -> anyhow::Result<String> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest as _, Sha256};

    // First create uncompressed tar to get diff_id
    let uncompressed_tar_path = output.with_extension("tar");
    {
        let tar_file = File::create(&uncompressed_tar_path)?;
        let mut tar = tar::Builder::new(tar_file);
        tar.append_dir_all(".", rootfs.as_std_path())?;
        tar.finish()?;
    }

    // Calculate diff_id (SHA256 of uncompressed tar)
    let uncompressed_bytes = fs::read(&uncompressed_tar_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&uncompressed_bytes);
    let diff_id = format!("sha256:{:x}", hasher.finalize());

    // Now compress it
    let tar_file = File::create(output)?;
    let mut encoder = GzEncoder::new(tar_file, Compression::default());
    std::io::copy(&mut std::io::Cursor::new(uncompressed_bytes), &mut encoder)?;
    encoder.finish()?;

    // Clean up uncompressed tar
    drop(fs::remove_file(&uncompressed_tar_path));

    Ok(diff_id)
}

/// Calculate SHA256 hex digest.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Create OCI image config.
fn create_image_config(diff_id: &str, oci_arch: &str) -> serde_json::Value {
    serde_json::json!({
        "architecture": oci_arch,
        "os": "linux",
        "config": {
            "Entrypoint": ["/usr/bin/bencher"],
            "Cmd": ["mock"],
            "WorkingDir": "/"
        },
        "rootfs": {
            "type": "layers",
            "diff_ids": [diff_id]
        },
        "history": []
    })
}

/// Create OCI manifest.
fn create_manifest(
    layer_digest: &str,
    layer_size: usize,
    config_digest: &str,
    config_size: usize,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": format!("sha256:{config_digest}"),
            "size": config_size
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "digest": format!("sha256:{layer_digest}"),
                "size": layer_size
            }
        ]
    })
}

/// Create OCI index.
fn create_index(manifest_digest: &str, manifest_size: usize, oci_arch: &str) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 2,
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": format!("sha256:{manifest_digest}"),
                "size": manifest_size,
                "platform": {
                    "architecture": oci_arch,
                    "os": "linux"
                }
            }
        ]
    })
}
