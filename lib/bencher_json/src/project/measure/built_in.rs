use crate::{JsonNewMeasure, MeasureNameId};

pub trait BuiltInMeasure {
    const NAME_STR: &'static str;
    const DISPLAY_STR: Option<&'static str>;
    const SLUG_STR: &'static str;
    const UNITS_STR: &'static str;

    #[expect(clippy::expect_used)]
    fn name_id() -> MeasureNameId {
        Self::SLUG_STR
            .parse()
            .expect("Failed to parse measure slug.")
    }

    fn from_str(measure_str: &str) -> Option<JsonNewMeasure> {
        (Self::DISPLAY_STR.is_some_and(|display| measure_str == display)
            || measure_str == Self::NAME_STR
            || measure_str == Self::SLUG_STR)
            .then(Self::new_json)
    }

    #[expect(clippy::expect_used)]
    fn new_json() -> JsonNewMeasure {
        JsonNewMeasure {
            name: Self::DISPLAY_STR
                .unwrap_or(Self::NAME_STR)
                .parse()
                .expect("Failed to parse measure name."),
            slug: Some(
                Self::SLUG_STR
                    .parse()
                    .expect("Failed to parse measure slug."),
            ),
            units: Self::UNITS_STR
                .parse()
                .expect("Failed to parse measure units."),
        }
    }
}

macro_rules! create_measure {
    ($id:ident, $name:literal, $slug:literal, $units:expr) => {
        pub struct $id;

        impl crate::project::measure::built_in::BuiltInMeasure for $id {
            const NAME_STR: &'static str = $name;
            const DISPLAY_STR: Option<&'static str> = None;
            const SLUG_STR: &'static str = $slug;
            const UNITS_STR: &'static str = $units;
        }
    };
    ($id:ident, $name:literal, $display:literal, $slug:literal, $units:expr) => {
        pub struct $id;

        impl crate::project::measure::built_in::BuiltInMeasure for $id {
            const NAME_STR: &'static str = $name;
            const DISPLAY_STR: Option<&'static str> = Some($display);
            const SLUG_STR: &'static str = $slug;
            const UNITS_STR: &'static str = $units;
        }
    };
}

pub mod default {
    use bencher_valid::NANOSECONDS;

    create_measure!(Latency, "Latency", "latency", NANOSECONDS);
    create_measure!(
        Throughput,
        "Throughput",
        "throughput",
        "operations / second (ops/s)"
    );
}

pub mod json {
    use bencher_valid::{BYTES, SECONDS};

    create_measure!(BuildTime, "Build Time", "build-time", SECONDS);
    create_measure!(FileSize, "File Size", "file-size", BYTES);
}

pub mod iai {
    create_measure!(Instructions, "Instructions", "instructions", "instructions");
    create_measure!(L1Accesses, "L1 Accesses", "l1-accesses", "accesses");
    create_measure!(L2Accesses, "L2 Accesses", "l2-accesses", "accesses");
    create_measure!(RamAccesses, "RAM Accesses", "ram-accesses", "accesses");
    create_measure!(
        EstimatedCycles,
        "Estimated Cycles",
        "estimated-cycles",
        "cycles"
    );
}

pub mod gungraun {
    use bencher_valid::BYTES;

    create_measure!(Unknown, "Unknown", "unknown-metric", "unknown");

    // Callgrind and Cachegrind
    create_measure!(Instructions, "Instructions", "instructions", "instructions");
    create_measure!(L1Hits, "L1 Hits", "l1-hits", "hits");
    create_measure!(L2Hits, "L2 Hits", "l2-hits", "hits");
    create_measure!(LLHits, "LL Hits", "ll-hits", "hits");
    create_measure!(RamHits, "RAM Hits", "ram-hits", "hits");
    create_measure!(
        TotalReadWrite,
        "Total read+write",
        "total-read-write",
        "reads/writes"
    );
    create_measure!(
        EstimatedCycles,
        "Estimated Cycles",
        "estimated-cycles",
        "cycles"
    );

    create_measure!(GlobalBusEvents, "Ge", "global-bus-events", "events");

    create_measure!(Dr, "Dr", "dr", "reads");
    create_measure!(Dw, "Dw", "dw", "writes");
    create_measure!(I1mr, "I1mr", "i1mr", "misses (reads)");
    create_measure!(D1mr, "D1mr", "d1mr", "misses (reads)");
    create_measure!(D1mw, "D1mw", "d1mw", "misses (writes)");
    create_measure!(ILmr, "ILmr", "ilmr", "misses (reads)");
    create_measure!(DLmr, "DLmr", "dlmr", "misses (reads)");
    create_measure!(DLmw, "DLmw", "dlmw", "misses (writes)");
    create_measure!(I1MissRate, "I1 Miss Rate", "i1-miss-rate", "misses (%)");
    create_measure!(LLiMissRate, "LLi Miss Rate", "lli-miss-rate", "misses (%)");
    create_measure!(D1MissRate, "D1 Miss Rate", "d1-miss-rate", "misses (%)");
    create_measure!(LLdMissRate, "LLd Miss Rate", "lld-miss-rate", "misses (%)");
    create_measure!(LLMissRate, "LL Miss Rate", "ll-miss-rate", "misses (%)");
    create_measure!(L1HitRate, "L1 Hit Rate", "l1-hit-rate", "hits (%)");
    create_measure!(LLHitRate, "LL Hit Rate", "ll-hit-rate", "hits (%)");
    create_measure!(RamHitRate, "RAM Hit Rate", "ram-hit-rate", "hits (%)");
    create_measure!(SysCount, "SysCount", "sys-count", "syscalls");
    create_measure!(SysTime, "SysTime", "sys-time", "msec/usec/nsec");
    create_measure!(SysCpuTime, "SysCpuTime", "sys-cpu-time", "nsec");
    create_measure!(Bc, "Bc", "bc", "branches");
    create_measure!(Bcm, "Bcm", "bcm", "branches");
    create_measure!(Bi, "Bi", "bi", "branches");
    create_measure!(Bim, "Bim", "bim", "branches");
    create_measure!(ILdmr, "ILdmr", "ildmr", "misses (reads)");
    create_measure!(DLdmr, "DLdmr", "dldmr", "misses (reads)");
    create_measure!(DLdmw, "DLdmw", "dldmw", "misses (writes)");
    create_measure!(AcCost1, "AcCost1", "accost1", "temporal locality count");
    create_measure!(AcCost2, "AcCost2", "accost2", "temporal locality count");
    create_measure!(SpLoss1, "SpLoss1", "sploss1", "spatial loss count");
    create_measure!(SpLoss2, "SpLoss2", "sploss2", "spatial loss count");

    // DHAT
    create_measure!(TotalUnits, "Total units", "total-units", "units");
    create_measure!(TotalEvents, "Total events", "total-events", "events");
    create_measure!(TotalBytes, "Total bytes", "total-bytes", BYTES);
    create_measure!(TotalBlocks, "Total blocks", "total-blocks", "blocks");
    create_measure!(AtTGmaxBytes, "At t-gmax bytes", "at-t-gmax-bytes", BYTES);
    create_measure!(
        AtTGmaxBlocks,
        "At t-gmax blocks",
        "at-t-gmax-blocks",
        "blocks"
    );
    create_measure!(AtTEndBytes, "At t-end bytes", "at-t-end-bytes", BYTES);
    create_measure!(AtTEndBlocks, "At t-end blocks", "at-t-end-blocks", "blocks");
    create_measure!(ReadsBytes, "Reads bytes", "reads-bytes", BYTES);
    create_measure!(WritesBytes, "Writes bytes", "writes-bytes", BYTES);
    create_measure!(
        TotalLifetimes,
        "Total lifetimes",
        "total-lifetimes",
        "lifetimes"
    );
    create_measure!(MaximumBytes, "Maximum bytes", "maximum-bytes", BYTES);
    create_measure!(MaximumBlocks, "Maximum blocks", "maximum-blocks", "blocks");

    // Memcheck
    create_measure!(
        MemcheckErrors,
        "Errors",
        "Memcheck Errors",
        "memcheck-errors",
        "errors"
    );
    create_measure!(
        MemcheckContexts,
        "Contexts",
        "Memcheck Contexts",
        "memcheck-contexts",
        "contexts"
    );
    create_measure!(
        MemcheckSuppressedErrors,
        "Suppressed Errors",
        "Memcheck Suppressed Errors",
        "memcheck-suppressed-errors",
        "suppressed errors"
    );
    create_measure!(
        MemcheckSuppressedContexts,
        "Suppressed Contexts",
        "Memcheck Suppressed Contexts",
        "memcheck-suppressed-contexts",
        "suppressed contexts"
    );

    // Helgrind
    create_measure!(
        HelgrindErrors,
        "Errors",
        "Helgrind Errors",
        "helgrind-errors",
        "errors"
    );
    create_measure!(
        HelgrindContexts,
        "Contexts",
        "Helgrind Contexts",
        "helgrind-contexts",
        "contexts"
    );
    create_measure!(
        HelgrindSuppressedErrors,
        "Suppressed Errors",
        "Helgrind Suppressed Errors",
        "helgrind-suppressed-errors",
        "suppressed errors"
    );
    create_measure!(
        HelgrindSuppressedContexts,
        "Suppressed Contexts",
        "Helgrind Suppressed Contexts",
        "helgrind-suppressed-contexts",
        "suppressed contexts"
    );

    // Drd
    create_measure!(DrdErrors, "Errors", "DRD Errors", "drd-errors", "errors");
    create_measure!(
        DrdContexts,
        "Contexts",
        "DRD Contexts",
        "drd-contexts",
        "contexts"
    );
    create_measure!(
        DrdSuppressedErrors,
        "Suppressed Errors",
        "DRD Suppressed Errors",
        "drd-suppressed-errors",
        "suppressed errors"
    );
    create_measure!(
        DrdSuppressedContexts,
        "Suppressed Contexts",
        "DRD Suppressed Contexts",
        "drd-suppressed-contexts",
        "suppressed contexts"
    );
}
