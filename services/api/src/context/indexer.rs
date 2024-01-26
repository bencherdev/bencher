#![cfg(feature = "plus")]

use bencher_google_index::GoogleIndex;
use bencher_json::system::config::{JsonGoogleIndex, JsonIndex};
use url::Url;

use crate::config::plus::PlusError;

pub struct Indexer {
    pub google: GoogleIndex,
}

impl TryFrom<JsonIndex> for Indexer {
    type Error = PlusError;

    fn try_from(index: JsonIndex) -> Result<Self, Self::Error> {
        let JsonIndex { google } = index;

        let JsonGoogleIndex {
            private_key,
            client_email,
            token_uri,
        } = google;
        let google = GoogleIndex::new(private_key.into(), client_email.into(), token_uri.into())
            .map_err(PlusError::GoogleIndex)?;

        Ok(Self { google })
    }
}

impl Indexer {
    pub async fn updated(&self, url: Url) -> Result<(), PlusError> {
        self.google
            .url_updated(url)
            .await
            .map_err(PlusError::GoogleIndex)
    }

    pub async fn deleted(&self, url: Url) -> Result<(), PlusError> {
        self.google
            .url_deleted(url)
            .await
            .map_err(PlusError::GoogleIndex)
    }
}
