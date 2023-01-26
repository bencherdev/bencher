use bencher_json::{
    system::jwt::{DecodingKey, EncodingKey},
    Secret,
};

pub struct SecretKey {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl From<Secret> for SecretKey {
    fn from(secret_key: Secret) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret_key.as_ref().as_bytes()),
            decoding: DecodingKey::from_secret(secret_key.as_ref().as_bytes()),
        }
    }
}
