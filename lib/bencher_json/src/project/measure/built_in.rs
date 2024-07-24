use crate::JsonNewMeasure;
use bencher_valid::NameId;

pub trait BuiltInMeasure {
    const NAME_STR: &'static str;
    const SLUG_STR: &'static str;
    const UNITS_STR: &'static str;

    fn name_id() -> NameId {
        Self::SLUG_STR
            .parse()
            .expect("Failed to parse measure slug.")
    }

    fn from_name_id(measure_str: &str) -> Option<JsonNewMeasure> {
        (measure_str == Self::NAME_STR || measure_str == Self::SLUG_STR).then(Self::new_json)
    }

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
    ($id:ident, $name:literal, $slug:literal, $units:literal) => {
        pub struct $id;

        impl crate::project::measure::built_in::BuiltInMeasure for $id {
            const NAME_STR: &'static str = $name;
            const SLUG_STR: &'static str = $slug;
            const UNITS_STR: &'static str = $units;
        }
    };
}

pub mod generic {
    create_measure!(Latency, "Latency", "latency", "nanoseconds (ns)");

    create_measure!(
        Throughput,
        "Throughput",
        "throughput",
        "operations / second (ops/s)"
    );
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
    pub mod callgrind_tool {
        create_measure!(Instructions, "Instructions", "instructions", "instructions");

        create_measure!(L1Hits, "L1 Hits", "l1-hits", "hits");

        create_measure!(L2Hits, "L2 Hits", "l2-hits", "hits");

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
    }

    pub mod dhat_tool {
        create_measure!(TotalBytes, "Total bytes", "total-bytes", "bytes (B)");

        create_measure!(TotalBlocks, "Total blocks", "total-blocks", "blocks");

        create_measure!(
            AtTGmaxBytes,
            "At t-gmax bytes",
            "at-t-gmax-bytes",
            "bytes (B)"
        );

        create_measure!(
            AtTGmaxBlocks,
            "At t-gmax blocks",
            "at-t-gmax-blocks",
            "blocks"
        );

        create_measure!(AtTEndBytes, "At t-end bytes", "at-t-end-bytes", "bytes (B)");

        create_measure!(AtTEndBlocks, "At t-end blocks", "at-t-end-blocks", "blocks");

        create_measure!(ReadsBytes, "Reads bytes", "reads-bytes", "bytes (B)");

        create_measure!(WritesBytes, "Writes bytes", "writes-bytes", "bytes (B)");
    }
}

pub mod file_size {
    create_measure!(FileSize, "File Size", "file-size", "bytes (B)");
}
