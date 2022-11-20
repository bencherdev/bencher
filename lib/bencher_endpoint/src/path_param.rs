use crate::ToEndpoint;

#[derive(Clone)]
pub struct PathParam<Resource>(pub String, pub Option<Resource>);

impl<Resource> ToEndpoint for PathParam<Resource>
where
    Resource: ToEndpoint,
{
    fn to_endpoint(&self) -> String {
        Self::resource(&self.0, &self.1)
    }
}
