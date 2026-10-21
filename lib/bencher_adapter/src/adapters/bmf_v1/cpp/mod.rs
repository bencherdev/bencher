pub mod catch2;
pub mod catch2_text;
pub mod catch2_xml;
pub mod google;

#[cfg(test)]
mod test_cpp_go_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings};

    /// Each C++ and Go harness's leaves, and the directory of its fixtures.
    const HARNESSES: [(&[Adapter], &str); 3] = [
        (&[Adapter::CppGoogle], "cpp/google"),
        (
            &[Adapter::CppCatch2Xml, Adapter::CppCatch2Text],
            "cpp/catch2",
        ),
        (&[Adapter::GoBenchJson, Adapter::GoBenchText], "go/bench"),
    ];

    /// Kills a leaf that claims another C++ or Go harness's output.
    #[test]
    fn cpp_go_v1_leaves_refuse_other_harnesses() {
        let settings = Settings::new(None, BmfVersion::V1);
        for (leaves, directory) in HARNESSES {
            for (_, other) in HARNESSES.iter().filter(|(_, other)| *other != directory) {
                for entry in std::fs::read_dir(format!("./tool_output/{other}"))
                    .expect("Failed to read the fixtures")
                {
                    let path = entry.expect("Failed to read a fixture").path();
                    let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
                    for leaf in leaves {
                        assert_eq!(
                            leaf.convert(&input, settings),
                            None,
                            "{leaf} claimed {}",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}
