use crate::JsonNewMeasure;
use bencher_valid::NameId;

pub trait BuiltInMeasure {
    const NAME_STR: &'static str;
    const SLUG_STR: &'static str;
    const UNITS_STR: &'static str;

    #[expect(clippy::expect_used)]
    fn name_id() -> NameId {
        Self::SLUG_STR
            .parse()
            .expect("Failed to parse measure slug.")
    }

    fn from_str(measure_str: &str) -> Option<JsonNewMeasure> {
        (measure_str == Self::NAME_STR || measure_str == Self::SLUG_STR).then(Self::new_json)
    }

    #[expect(clippy::expect_used)]
    fn new_json() -> JsonNewMeasure {
        JsonNewMeasure {
            name: Self::NAME_STR
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

pub mod iai_callgrind {
    use bencher_valid::BYTES;

    create_measure!(Unknown, "Unknown", "unknown-metric", "unknown");

    // Callgrind
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

    create_measure!(Dr, "Dr", "dr", "dr");
    create_measure!(Dw, "Dw", "dw", "dw");
    create_measure!(I1mr, "I1mr", "i1mr", "i1mr");
    create_measure!(D1mr, "D1mr", "d1mr", "d1mr");
    create_measure!(D1mw, "D1mw", "d1mw", "d1mw");
    create_measure!(ILmr, "ILmr", "ilmr", "ilmr");
    create_measure!(DLmr, "DLmr", "dlmr", "dlmr");
    create_measure!(DLmw, "DLmw", "dlmw", "dlmw");
    create_measure!(I1MissRate, "I1 Miss Rate", "i1-miss-rate", "missrate");
    create_measure!(LLiMissRate, "LLi Miss Rate", "lli-miss-rate", "missrate");
    create_measure!(D1MissRate, "D1 Miss Rate", "d1-miss-rate", "missrate");
    create_measure!(LLdMissRate, "LLd Miss Rate", "lld-miss-rate", "missrate");
    create_measure!(LLMissRate, "LL Miss Rate", "ll-miss-rate", "missrate");
    create_measure!(L1HitRate, "L1 Hit Rate", "l1-hit-rate", "hitrate");
    create_measure!(LLHitRate, "LL Hit Rate", "ll-hit-rate", "hitrate");
    create_measure!(RamHitRate, "RAM Hit Rate", "ram-hit-rate", "hitrate");
    create_measure!(SysCount, "SysCount", "sys-count", "sys");
    create_measure!(SysTime, "SysTime", "sys-time", "sys");
    create_measure!(SysCpuTime, "SysCpuTime", "sys-cpu-time", "sys");
    create_measure!(Bc, "Bc", "bc", "bc");
    create_measure!(Bcm, "Bcm", "bcm", "bcm");
    create_measure!(Bi, "Bi", "bi", "bi");
    create_measure!(Bim, "Bim", "bim", "bim");
    create_measure!(ILdmr, "ILdmr", "ildmr", "ildmr");
    create_measure!(DLdmr, "DLdmr", "dldmr", "dldmr");
    create_measure!(DLdmw, "DLdmw", "dldmw", "dldmw");
    create_measure!(AcCost1, "AcCost1", "accost1", "accost1");
    create_measure!(AcCost2, "AcCost2", "accost2", "accost2");
    create_measure!(SpLoss1, "SpLoss1", "sploss1", "sploss1");
    create_measure!(SpLoss2, "SpLoss2", "sploss2", "sploss2");

    // DHAT
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

    // Memcheck
    create_measure!(MemcheckErrors, "Errors", "memcheck-errors", "errors");
    create_measure!(
        MemcheckContexts,
        "Contexts",
        "memcheck-contexts",
        "contexts"
    );
    create_measure!(
        MemcheckSuppressedErrors,
        "Suppressed Errors",
        "memcheck-suppressed-errors",
        "suppressed errors"
    );
    create_measure!(
        MemcheckSuppressedContexts,
        "Suppressed Contexts",
        "memcheck-suppressed-contexts",
        "suppressed contexts"
    );
}
