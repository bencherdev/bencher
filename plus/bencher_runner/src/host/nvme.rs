//! `NVMe` controllers and their SMART / health log page.

use std::io;

use bencher_json::runner::{
    HealthFindingKind, HealthState, JsonHealthFinding, JsonNvmeHealth, JsonNvmeHealthLog,
};
use camino::Utf8Path;

/// The SMART / health information log page, in bytes.
pub(crate) const SMART_LOG_LEN: usize = 512;
/// The critical warning bit for a temperature past a threshold, which clears by itself.
const TEMPERATURE: u8 = 1 << 1;
const PERCENTAGE_USED_WARNING: u8 = 80;
/// Media and data integrity errors, the 11th 16-byte counter of the page.
const MEDIA_ERRORS_COUNTER: usize = 10;

/// A controller, and why its log could not be read.
pub(crate) struct Controller {
    pub health: JsonNvmeHealth,
    pub error: Option<io::Error>,
}

/// Every `NVMe` controller under `sysfs`, by name, with its log read through
/// `read_log` from its device under `dev`.
pub(crate) fn controllers<F>(sysfs: &Utf8Path, dev: &Utf8Path, read_log: F) -> Vec<Controller>
where
    F: Fn(&Utf8Path) -> io::Result<[u8; SMART_LOG_LEN]>,
{
    let class = sysfs.join("class/nvme");
    let Ok(entries) = class.read_dir_utf8() else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_owned())
        .filter(|name| name.starts_with("nvme"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let read = |attribute: &str| {
                std::fs::read_to_string(class.join(&name).join(attribute))
                    .ok()
                    .map(|value| value.trim().to_owned())
            };
            let (log, error) = match read_log(&dev.join(&name)) {
                Ok(page) => (Some(parse_log(&page)), None),
                Err(error) => (None, Some(error)),
            };
            Controller {
                health: JsonNvmeHealth {
                    model: read("model"),
                    serial: read("serial"),
                    name,
                    log,
                },
                error,
            }
        })
        .collect()
}

/// What is wrong with a controller, by its log.
pub(crate) fn findings(name: &str, log: &JsonNvmeHealthLog) -> Vec<JsonHealthFinding> {
    let finding = |kind, state| JsonHealthFinding {
        device: name.to_owned(),
        kind,
        state,
    };
    let mut findings = Vec::new();
    if log.critical_warning & !TEMPERATURE != 0 {
        findings.push(finding(
            HealthFindingKind::CriticalWarning,
            HealthState::Failing,
        ));
    }
    if log.critical_warning & TEMPERATURE != 0 {
        findings.push(finding(
            HealthFindingKind::Temperature,
            HealthState::Warning,
        ));
    }
    if log.available_spare < log.available_spare_threshold {
        findings.push(finding(
            HealthFindingKind::AvailableSpare,
            HealthState::Failing,
        ));
    } else if u16::from(log.available_spare) <= 2 * u16::from(log.available_spare_threshold) {
        findings.push(finding(
            HealthFindingKind::AvailableSpare,
            HealthState::Warning,
        ));
    }
    if log.percentage_used > PERCENTAGE_USED_WARNING {
        findings.push(finding(
            HealthFindingKind::PercentageUsed,
            HealthState::Warning,
        ));
    }
    if log.media_errors > 0 {
        findings.push(finding(
            HealthFindingKind::MediaErrors,
            HealthState::Warning,
        ));
    }
    findings
}

/// The fields that set a finding, from the little-endian page.
pub(crate) fn parse_log(page: &[u8; SMART_LOG_LEN]) -> JsonNvmeHealthLog {
    let [
        critical_warning,
        temperature_low,
        temperature_high,
        available_spare,
        available_spare_threshold,
        percentage_used,
        ..,
    ] = *page;
    let media_errors = page
        .as_chunks::<16>()
        .0
        .get(MEDIA_ERRORS_COUNTER)
        .map_or(0, |counter| {
            counter
                .iter()
                .rev()
                .fold(0, |value, byte| (value << 8) | u128::from(*byte))
        });
    JsonNvmeHealthLog {
        critical_warning,
        temperature: (u16::from(temperature_high) << 8) | u16::from(temperature_low),
        available_spare,
        available_spare_threshold,
        percentage_used,
        media_errors: u64::try_from(media_errors).unwrap_or(u64::MAX),
    }
}

/// Get Log Page for the SMART / health log, through the admin passthrough,
/// which needs `CAP_SYS_ADMIN`.
#[cfg(target_os = "linux")]
pub(crate) fn read_log(device: &Utf8Path) -> io::Result<[u8; SMART_LOG_LEN]> {
    use std::os::fd::AsRawFd as _;

    const GET_LOG_PAGE: u8 = 0x02;
    const SMART_LOG: u32 = 0x02;
    const ALL_NAMESPACES: u32 = 0xffff_ffff;
    const DWORDS: u32 = 128;
    const _: () = assert!(SMART_LOG_LEN == 4 * 128, "the page is DWORDS long");

    let file = std::fs::File::open(device)?;
    let mut page = [0; SMART_LOG_LEN];
    // Exposed, since the kernel writes the page through this address.
    let addr = u64::try_from(page.as_mut_ptr().expose_provenance()).map_err(io::Error::other)?;
    let mut command = AdminCommand {
        opcode: GET_LOG_PAGE,
        nsid: ALL_NAMESPACES,
        addr,
        data_len: 4 * DWORDS,
        // The 0-based dword count, then the log identifier.
        cdw10: ((DWORDS - 1) << 16) | SMART_LOG,
        ..AdminCommand::default()
    };
    #[expect(unsafe_code, reason = "the admin passthrough is an ioctl")]
    // SAFETY: `command` is the kernel's `struct nvme_admin_cmd`, and `addr`
    // points at `page`, which outlives the call and holds `data_len` bytes.
    let status = unsafe { admin_command(file.as_raw_fd(), &raw mut command) }?;
    if status == 0 {
        Ok(page)
    } else {
        Err(io::Error::other(format!(
            "NVMe Get Log Page failed with status {status:#x}"
        )))
    }
}

/// Off Linux there is no `NVMe` passthrough.
#[cfg(not(target_os = "linux"))]
pub(crate) fn read_log(_device: &Utf8Path) -> io::Result<[u8; SMART_LOG_LEN]> {
    Err(io::ErrorKind::Unsupported.into())
}

/// The kernel's `struct nvme_admin_cmd` (`linux/nvme_ioctl.h`), which it reads whole.
#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Default)]
struct AdminCommand {
    opcode: u8,
    flags: u8,
    rsvd1: u16,
    nsid: u32,
    cdw2: u32,
    cdw3: u32,
    metadata: u64,
    addr: u64,
    metadata_len: u32,
    data_len: u32,
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
    timeout_ms: u32,
    result: u32,
}

#[cfg(target_os = "linux")]
const _: () = assert!(
    size_of::<AdminCommand>() == 72,
    "the kernel's command is 72 bytes"
);

#[cfg(target_os = "linux")]
nix::ioctl_readwrite!(admin_command, b'N', 0x41, AdminCommand);

#[cfg(test)]
pub(crate) mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    /// A page with `bytes` written at their offsets over an otherwise healthy log.
    pub(crate) fn page(bytes: &[(usize, u8)]) -> [u8; SMART_LOG_LEN] {
        let mut page = [0; SMART_LOG_LEN];
        // Composite temperature 310 K, spare 100% over a 10% threshold, 3% used.
        for (offset, byte) in [(1, 0x36), (2, 0x01), (3, 100), (4, 10), (5, 3)]
            .into_iter()
            .chain(bytes.iter().copied())
        {
            if let Some(slot) = page.get_mut(offset) {
                *slot = byte;
            }
        }
        page
    }

    fn kinds(page: &[u8; SMART_LOG_LEN]) -> Vec<(HealthFindingKind, HealthState)> {
        findings("nvme0", &parse_log(page))
            .into_iter()
            .map(|finding| (finding.kind, finding.state))
            .collect()
    }

    #[test]
    fn a_page_parses_little_endian_at_its_offsets() {
        // Kills a field read from the wrong offset or in the wrong byte order.
        let log = parse_log(&page(&[(0, 0x04), (160, 0x02), (161, 0x01)]));
        assert_eq!(
            log,
            JsonNvmeHealthLog {
                critical_warning: 0x04,
                temperature: 310,
                available_spare: 100,
                available_spare_threshold: 10,
                percentage_used: 3,
                media_errors: 0x0102,
            }
        );
    }

    #[test]
    fn a_media_error_count_past_u64_saturates() {
        // Kills a count that wraps to a small number past 64 bits.
        let log = parse_log(&page(&[(168, 1)]));
        assert_eq!(log.media_errors, u64::MAX);
    }

    #[test]
    fn a_healthy_page_has_no_findings() {
        // Kills a rule that fires on a healthy controller.
        assert_eq!(kinds(&page(&[])), []);
    }

    #[test]
    fn each_critical_warning_bit_but_temperature_is_failing() {
        // Kills a critical bit that is missed, or a temperature bit that is failing.
        for bit in [0, 2, 3, 4, 5, 6, 7] {
            let found = kinds(&page(&[(0, 1 << bit)]));
            assert!(
                found.contains(&(HealthFindingKind::CriticalWarning, HealthState::Failing)),
                "bit {bit}: {found:?}"
            );
        }
        assert_eq!(
            kinds(&page(&[(0, TEMPERATURE)])),
            [(HealthFindingKind::Temperature, HealthState::Warning)]
        );
    }

    #[test]
    fn spare_at_twice_its_threshold_warns_and_below_it_fails() {
        // Kills the spare boundaries moved by one either way.
        let spare = |spare| kinds(&page(&[(3, spare)]));
        assert_eq!(spare(21), []);
        assert_eq!(
            spare(20),
            [(HealthFindingKind::AvailableSpare, HealthState::Warning)]
        );
        assert_eq!(
            spare(10),
            [(HealthFindingKind::AvailableSpare, HealthState::Warning)]
        );
        assert_eq!(
            spare(9),
            [(HealthFindingKind::AvailableSpare, HealthState::Failing)]
        );
    }

    #[test]
    fn percentage_used_over_80_warns() {
        // Kills the endurance boundary moved by one either way.
        assert_eq!(kinds(&page(&[(5, 80)])), []);
        assert_eq!(
            kinds(&page(&[(5, 81)])),
            [(HealthFindingKind::PercentageUsed, HealthState::Warning)]
        );
    }

    #[test]
    fn any_media_error_warns() {
        // Kills media errors that are ignored, or failing rather than a warning.
        assert_eq!(
            kinds(&page(&[(160, 1)])),
            [(HealthFindingKind::MediaErrors, HealthState::Warning)]
        );
    }

    #[test]
    fn an_unreadable_log_still_lists_the_controller() {
        // Kills a controller dropped when its log cannot be read, as by a
        // runner that is not root, and a log read from another device.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_owned()).unwrap();
        for (name, model) in [("nvme1", "Disk B"), ("nvme0", "Disk A")] {
            let class = root.join("class/nvme").join(name);
            std::fs::create_dir_all(&class).unwrap();
            std::fs::write(class.join("model"), format!("{model}   \n")).unwrap();
            std::fs::write(class.join("serial"), "S123\n").unwrap();
        }
        let dev = root.join("dev");

        let read = controllers(&root, &dev, |device| {
            if device == dev.join("nvme0") {
                Ok(page(&[]))
            } else {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        });
        let health: Vec<_> = read.iter().map(|controller| &controller.health).collect();
        assert_eq!(
            health,
            [
                &JsonNvmeHealth {
                    name: "nvme0".to_owned(),
                    model: Some("Disk A".to_owned()),
                    serial: Some("S123".to_owned()),
                    log: Some(parse_log(&page(&[]))),
                },
                &JsonNvmeHealth {
                    name: "nvme1".to_owned(),
                    model: Some("Disk B".to_owned()),
                    serial: Some("S123".to_owned()),
                    log: None,
                },
            ]
        );
        assert!(read[0].error.is_none());
        assert!(read[1].error.is_some());
    }
}
