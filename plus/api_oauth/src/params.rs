use std::{borrow::Cow, collections::HashMap, str::FromStr};

use url::form_urlencoded;

/// The parameters of a request, each one this server knows given at most once.
/// A repeat of any other parameter is ignored, as the parameter itself is.
pub(crate) struct Params<'q>(HashMap<Cow<'q, str>, Cow<'q, str>>);

impl<'q> Params<'q> {
    /// Parse a query or form body, unless it repeats one of the `known` parameters.
    pub(crate) fn parse(input: &'q [u8], known: &[&str]) -> Option<Self> {
        let mut params = HashMap::new();
        for (name, value) in form_urlencoded::parse(input) {
            let is_known = known.contains(&name.as_ref());
            if params.insert(name, value).is_some() && is_known {
                return None;
            }
        }
        Some(Self(params))
    }

    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(AsRef::as_ref)
    }

    /// The parameter, if it is given, unless it is invalid.
    pub(crate) fn value<T>(&self, name: &str) -> Result<Option<T>, ()>
    where
        T: FromStr,
    {
        self.get(name)
            .map(|value| value.parse::<T>().map_err(|_err| ()))
            .transpose()
    }
}
