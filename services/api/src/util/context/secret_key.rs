use bencher_json::system::jwt::{DecodingKey, EncodingKey};

pub struct SecretKey {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl From<String> for SecretKey {
    fn from(secret_key: String) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret_key.as_str().as_bytes()),
            decoding: DecodingKey::from_secret(secret_key.as_str().as_bytes()),
        }
    }
}
