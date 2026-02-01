//! aarch64 kernel loading and boot protocol.
//!
//! This module implements the ARM64 boot protocol for loading Image kernels.
//! ARM64 uses a device tree blob (DTB) instead of boot_params.

use std::fs::File;
use std::io::Read;

use camino::Utf8Path;
use linux_loader::loader::pe::PE as PeLoader;
use linux_loader::loader::KernelLoader;
use vm_fdt::{FdtWriter, FdtWriterResult};
use vm_memory::{GuestAddress, GuestMemory, GuestMemoryMmap};

use crate::error::VmmError;
use crate::gic::{Gic, GicVersion, GIC_NR_IRQS};
use crate::vcpu::aarch64::{DTB_ADDR, KERNEL_LOAD_ADDR};

use super::KernelEntry;

/// Serial port base address for PL011 UART.
const PL011_BASE: u64 = 0x0900_0000;

/// Serial port size.
const PL011_SIZE: u64 = 0x1000;

/// Serial port IRQ (SPI 1 = 32 + 1 = 33).
const PL011_IRQ: u32 = 33;

/// RTC base address for PL031.
const PL031_RTC_BASE: u64 = 0x0901_0000;

/// RTC size.
const PL031_RTC_SIZE: u64 = 0x1000;

/// RTC IRQ (SPI 2 = 32 + 2 = 34).
const PL031_RTC_IRQ: u32 = 34;

/// Maximum size for the device tree blob.
const FDT_MAX_SIZE: usize = 0x20_0000; // 2 MiB

/// Load an aarch64 Linux kernel.
pub fn load_kernel(
    guest_memory: &GuestMemoryMmap,
    kernel_path: &Utf8Path,
    cmdline: &str,
    vcpu_count: u32,
    memory_size: u64,
    gic: &Gic,
) -> Result<KernelEntry, VmmError> {
    // Open the kernel file
    let mut kernel_file =
        File::open(kernel_path).map_err(|e| VmmError::KernelLoad(e.to_string()))?;

    // Load kernel image
    let entry_addr = load_kernel_image(guest_memory, &mut kernel_file)?;

    // Setup device tree
    setup_device_tree(guest_memory, cmdline, vcpu_count, memory_size, gic)?;

    Ok(KernelEntry { entry_addr })
}

/// Load the kernel image into guest memory.
fn load_kernel_image(
    guest_memory: &GuestMemoryMmap,
    kernel_file: &mut File,
) -> Result<u64, VmmError> {
    // Try to load as PE (ARM64 Image format)
    let result = PeLoader::load(
        guest_memory,
        None,
        kernel_file,
        Some(GuestAddress(KERNEL_LOAD_ADDR)),
    );

    match result {
        Ok(kernel_entry) => Ok(kernel_entry.kernel_load.0),
        Err(_) => {
            // Fall back to raw image loading
            use std::io::Seek;
            kernel_file
                .seek(std::io::SeekFrom::Start(0))
                .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

            let mut kernel_data = Vec::new();
            kernel_file
                .read_to_end(&mut kernel_data)
                .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

            guest_memory
                .write_slice(&kernel_data, GuestAddress(KERNEL_LOAD_ADDR))
                .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

            Ok(KERNEL_LOAD_ADDR)
        }
    }
}

/// Setup the device tree blob for ARM64.
///
/// This creates a device tree with:
/// - CPU configuration
/// - Memory configuration
/// - GIC interrupt controller
/// - Serial port (PL011)
/// - Chosen node with command line
fn setup_device_tree(
    guest_memory: &GuestMemoryMmap,
    cmdline: &str,
    vcpu_count: u32,
    memory_size: u64,
    gic: &Gic,
) -> Result<(), VmmError> {
    let fdt_blob = create_fdt(cmdline, vcpu_count, memory_size, gic)
        .map_err(|e| VmmError::Boot(format!("Failed to create FDT: {e:?}")))?;

    // Write the FDT to guest memory
    guest_memory
        .write_slice(&fdt_blob, GuestAddress(DTB_ADDR))
        .map_err(|e| VmmError::Boot(format!("Failed to write FDT to memory: {e}")))?;

    Ok(())
}

/// Create the Flattened Device Tree.
fn create_fdt(
    cmdline: &str,
    vcpu_count: u32,
    memory_size: u64,
    gic: &Gic,
) -> FdtWriterResult<Vec<u8>> {
    let mut fdt = FdtWriter::new()?;

    // Root node
    let root = fdt.begin_node("")?;
    fdt.property_string("compatible", "linux,dummy-virt")?;
    fdt.property_u32("#address-cells", 2)?;
    fdt.property_u32("#size-cells", 2)?;
    fdt.property_u32("interrupt-parent", 1)?; // phandle of GIC

    // Chosen node (contains command line)
    create_chosen_node(&mut fdt, cmdline)?;

    // Memory node
    create_memory_node(&mut fdt, memory_size)?;

    // CPU nodes
    create_cpu_nodes(&mut fdt, vcpu_count)?;

    // GIC interrupt controller
    create_gic_node(&mut fdt, gic)?;

    // Timer node
    create_timer_node(&mut fdt)?;

    // Serial port (PL011 UART)
    create_serial_node(&mut fdt)?;

    // RTC (PL031)
    create_rtc_node(&mut fdt)?;

    fdt.end_node(root)?;

    fdt.finish()
}

/// Create the /chosen node with boot arguments.
fn create_chosen_node(fdt: &mut FdtWriter, cmdline: &str) -> FdtWriterResult<()> {
    let chosen = fdt.begin_node("chosen")?;
    fdt.property_string("bootargs", cmdline)?;
    // stdout-path points to the serial console
    fdt.property_string("stdout-path", "/pl011@9000000")?;
    fdt.end_node(chosen)?;
    Ok(())
}

/// Create the /memory node.
fn create_memory_node(fdt: &mut FdtWriter, memory_size: u64) -> FdtWriterResult<()> {
    let memory = fdt.begin_node("memory@80000000")?;
    fdt.property_string("device_type", "memory")?;
    // Memory starts at kernel load address and extends for memory_size
    fdt.property_array_u64("reg", &[KERNEL_LOAD_ADDR, memory_size])?;
    fdt.end_node(memory)?;
    Ok(())
}

/// Create the /cpus node and individual CPU nodes.
fn create_cpu_nodes(fdt: &mut FdtWriter, vcpu_count: u32) -> FdtWriterResult<()> {
    let cpus = fdt.begin_node("cpus")?;
    fdt.property_u32("#address-cells", 1)?;
    fdt.property_u32("#size-cells", 0)?;

    for cpu_id in 0..vcpu_count {
        let cpu_name = format!("cpu@{cpu_id}");
        let cpu = fdt.begin_node(&cpu_name)?;
        fdt.property_string("device_type", "cpu")?;
        fdt.property_string("compatible", "arm,arm-v8")?;
        fdt.property_string("enable-method", "psci")?;
        fdt.property_u32("reg", cpu_id)?;
        fdt.end_node(cpu)?;
    }

    fdt.end_node(cpus)?;
    Ok(())
}

/// Create the GIC interrupt controller node.
fn create_gic_node(fdt: &mut FdtWriter, gic: &Gic) -> FdtWriterResult<()> {
    let (compatible, reg) = match gic.version() {
        GicVersion::V3 => (
            "arm,gic-v3",
            vec![
                gic.dist_base(),
                gic.dist_size(),
                gic.cpu_base(),
                gic.cpu_size(),
            ],
        ),
        GicVersion::V2 => (
            "arm,cortex-a15-gic",
            vec![
                gic.dist_base(),
                gic.dist_size(),
                gic.cpu_base(),
                gic.cpu_size(),
            ],
        ),
    };

    let gic_name = format!("intc@{:x}", gic.dist_base());
    let gic_node = fdt.begin_node(&gic_name)?;
    fdt.property_string("compatible", compatible)?;
    fdt.property_null("interrupt-controller")?;
    fdt.property_u32("#interrupt-cells", 3)?;
    fdt.property_array_u64("reg", &reg)?;
    fdt.property_u32("phandle", 1)?;

    if gic.version() == GicVersion::V3 {
        fdt.property_null("redistributor-stride")?;
        fdt.property_u32("#redistributor-regions", 1)?;
    }

    fdt.end_node(gic_node)?;
    Ok(())
}

/// Create the timer node.
fn create_timer_node(fdt: &mut FdtWriter) -> FdtWriterResult<()> {
    let timer = fdt.begin_node("timer")?;
    fdt.property_string("compatible", "arm,armv8-timer")?;
    // Timer interrupts: secure phys, non-secure phys, virt, hyp
    // Format: <type, irq, flags> where type=1 is PPI
    fdt.property_array_u32(
        "interrupts",
        &[
            1, 13, 0x104, // Secure physical timer
            1, 14, 0x104, // Non-secure physical timer
            1, 11, 0x104, // Virtual timer
            1, 10, 0x104, // Hypervisor timer
        ],
    )?;
    fdt.property_null("always-on")?;
    fdt.end_node(timer)?;
    Ok(())
}

/// Create the serial port (PL011 UART) node.
fn create_serial_node(fdt: &mut FdtWriter) -> FdtWriterResult<()> {
    let serial_name = format!("pl011@{PL011_BASE:x}");
    let serial = fdt.begin_node(&serial_name)?;
    fdt.property_string("compatible", "arm,pl011\0arm,primecell")?;
    fdt.property_array_u64("reg", &[PL011_BASE, PL011_SIZE])?;
    // Interrupt: SPI type (0), IRQ number, level-triggered high
    fdt.property_array_u32("interrupts", &[0, PL011_IRQ - 32, 4])?;
    fdt.property_string("clock-names", "uartclk\0apb_pclk")?;
    // Clock references (dummy, kernel doesn't really need them for virtio)
    fdt.property_array_u32("clocks", &[0x8000, 0x8000])?;
    fdt.end_node(serial)?;
    Ok(())
}

/// Create the RTC (PL031) node.
fn create_rtc_node(fdt: &mut FdtWriter) -> FdtWriterResult<()> {
    let rtc_name = format!("pl031@{PL031_RTC_BASE:x}");
    let rtc = fdt.begin_node(&rtc_name)?;
    fdt.property_string("compatible", "arm,pl031\0arm,primecell")?;
    fdt.property_array_u64("reg", &[PL031_RTC_BASE, PL031_RTC_SIZE])?;
    fdt.property_array_u32("interrupts", &[0, PL031_RTC_IRQ - 32, 4])?;
    fdt.property_string("clock-names", "apb_pclk")?;
    fdt.property_array_u32("clocks", &[0x8000])?;
    fdt.end_node(rtc)?;
    Ok(())
}

/// Create a PSCI node for CPU power management.
fn create_psci_node(fdt: &mut FdtWriter) -> FdtWriterResult<()> {
    let psci = fdt.begin_node("psci")?;
    fdt.property_string("compatible", "arm,psci-1.0\0arm,psci-0.2\0arm,psci")?;
    fdt.property_string("method", "hvc")?;
    fdt.end_node(psci)?;
    Ok(())
}
