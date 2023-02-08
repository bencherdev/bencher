use bencher_valid::Secret;
use jsonwebtoken::{DecodingKey, EncodingKey};

use crate::LicenseError;

pub const PUBLIC_PEM: &str = include_str!("../public.pem");

pub enum Licensor {
    SelfHost {
        decoding: DecodingKey,
    },
    BencherCloud {
        encoding: EncodingKey,
        decoding: DecodingKey,
    },
}

impl Licensor {
    pub fn self_host() -> Result<Self, LicenseError> {
        let decoding = decoding_key()?;
        Ok(Self::SelfHost { decoding })
    }

    pub fn bencher_cloud(private_pem: Secret) -> Result<Self, LicenseError> {
        let encoding = encoding_key(private_pem.as_ref())?;
        let decoding = decoding_key()?;
        Ok(Self::BencherCloud { encoding, decoding })
    }
}

fn encoding_key(key: &str) -> Result<EncodingKey, LicenseError> {
    EncodingKey::from_ec_pem(key.as_bytes()).map_err(LicenseError::PrivatePem)
}

fn decoding_key() -> Result<DecodingKey, LicenseError> {
    DecodingKey::from_ec_pem(PUBLIC_PEM.as_bytes()).map_err(LicenseError::PublicPem)
}
