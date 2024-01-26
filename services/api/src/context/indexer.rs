#![cfg(feature = "plus")]

use bencher_bing_index::BingIndex;
use bencher_google_index::GoogleIndex;
use bencher_json::system::config::{JsonBingIndex, JsonGoogleIndex, JsonIndex};
use url::Url;

use crate::config::plus::PlusError;

pub struct Indexer {
    pub bing: BingIndex,
    pub google: GoogleIndex,
}

impl TryFrom<JsonIndex> for Indexer {
    type Error = PlusError;

    fn try_from(index: JsonIndex) -> Result<Self, Self::Error> {
        let JsonIndex { bing, google } = index;

        let JsonBingIndex { key, key_location } = bing;
        let bing = BingIndex::new(
            key.into(),
            key_location
                .map(TryInto::try_into)
                .transpose()
                .map_err(PlusError::KeyLocation)?,
        );

        let JsonGoogleIndex {
            private_key,
            client_email,
            token_uri,
        } = google;
        let google = GoogleIndex::new(private_key.into(), client_email.into(), token_uri.into())
            .map_err(PlusError::GoogleIndex)?;

        Ok(Self { bing, google })
    }
}

impl Indexer {
    pub async fn updated(&self, url: Url) -> Result<(), PlusError> {
        let bing = self
            .bing
            .index_now(&url)
            .await
            .map_err(PlusError::BingIndex);

        let google = self
            .google
            .url_updated(url)
            .await
            .map_err(PlusError::GoogleIndex);

        if bing.is_err() {
            bing
        } else {
            google
        }
    }

    pub async fn deleted(&self, url: Url) -> Result<(), PlusError> {
        let bing = self
            .bing
            .index_now(&url)
            .await
            .map_err(PlusError::BingIndex);

        let google = self
            .google
            .url_deleted(url)
            .await
            .map_err(PlusError::GoogleIndex);

        if bing.is_err() {
            bing
        } else {
            google
        }
    }
}
