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

macro_rules! impl_display {
    ($resource:ident) => {
        impl std::fmt::Display for $resource {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_endpoint())
            }
        }
    };
}

pub(crate) use impl_display;
