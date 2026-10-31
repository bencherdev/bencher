pub mod jmh;

#[cfg(test)]
mod test_jvm_dot_net_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings};

    /// Each harness's leaf, and the directory of its fixtures.
    const HARNESSES: [(Adapter, &str); 2] = [
        (Adapter::JavaJmh, "java/jmh"),
        (Adapter::CSharpDotNet, "c_sharp/dot_net"),
    ];

    /// Kills a leaf that claims the other harness's output.
    #[test]
    fn jvm_dot_net_v1_leaves_refuse_each_other() {
        let settings = Settings::new(None, BmfVersion::V1);
        for (leaf, directory) in HARNESSES {
            for (_, other) in HARNESSES.iter().filter(|(_, other)| *other != directory) {
                for entry in std::fs::read_dir(format!("./tool_output/{other}"))
                    .expect("Failed to read the fixtures")
                {
                    let path = entry.expect("Failed to read a fixture").path();
                    let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
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
