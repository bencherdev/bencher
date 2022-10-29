pub trait ToEndpoint {
    fn to_endpoint(&self) -> String;

    fn resource<Resource>(resource_str: impl AsRef<str>, resource: &Resource) -> String
    where
        Resource: ToEndpoint,
    {
        format!("{}{}", resource_str.as_ref(), resource.to_endpoint())
    }
}

impl<T> ToEndpoint for Option<T>
where
    T: ToEndpoint,
{
    fn to_endpoint(&self) -> String {
        if let Some(t) = self {
            Self::resource("/", t)
        } else {
            String::default()
        }
    }
}
