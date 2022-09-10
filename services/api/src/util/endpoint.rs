macro_rules! method_into_endpoint {
    ($parent:ident, $variant:ident, $method:ident) => {
        impl crate::IntoEndpoint for $method {
            fn into_endpoint(self) -> crate::Endpoint {
                $parent::$variant(self).into_endpoint()
            }
        }
    };
}

pub(crate) use method_into_endpoint;

macro_rules! into_endpoint {
    ($variant:ident, $endpoint:ident) => {
        impl crate::IntoEndpoint for $endpoint {
            fn into_endpoint(self) -> crate::Endpoint {
                crate::Endpoint::$variant(self)
            }
        }
    };
}

pub(crate) use into_endpoint;
