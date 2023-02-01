pub mod dot_net;

use crate::{Adapter, AdapterResults};
use dot_net::AdapterCSharpDotNet;

pub struct AdapterCSharp;

impl Adapter for AdapterCSharp {
    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterCSharpDotNet::parse(input)
    }
}

#[cfg(test)]
mod test_c_sharp {
    use super::AdapterCSharp;
    use crate::adapters::{c_sharp::dot_net::test_c_sharp_dot_net, test_util::convert_file_path};

    #[test]
    fn test_adapter_c_sharp_dot_net() {
        let results = convert_file_path::<AdapterCSharp>("./tool_output/c_sharp/dot_net/two.json");
        test_c_sharp_dot_net::validate_adapter_c_sharp_dot_net(results);
    }
}
