use crate::ApiError;

pub struct SecretKey(String);

impl ToString for SecretKey {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl AsRef<str> for SecretKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SecretKey {
    type Error = ApiError;

    fn try_from(secret_key: String) -> Result<Self, Self::Error> {
        Ok(Self(secret_key))
    }
}
