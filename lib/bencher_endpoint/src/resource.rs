use crate::ToEndpoint;

#[derive(Clone, Copy)]
pub struct Resource;

impl ToEndpoint for Resource {
    fn to_endpoint(&self) -> String {
        String::default()
    }
}
