use bencher_bing_index::BingIndex;
use bencher_google_index::GoogleIndex;
use bencher_json::system::config::{JsonBingIndex, JsonGoogleIndex, JsonIndex};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Failed to parse Bing Index key location: {0}")]
    KeyLocation(bencher_json::ValidError),
    #[error("Bing Index failed: {0}")]
    BingIndex(bencher_bing_index::BingIndexError),
    #[error("Google Index failed: {0}")]
    GoogleIndex(bencher_google_index::GoogleIndexError),
}

pub struct Indexer {
    pub bing: BingIndex,
    pub google: GoogleIndex,
}

impl TryFrom<JsonIndex> for Indexer {
    type Error = IndexError;

    fn try_from(index: JsonIndex) -> Result<Self, Self::Error> {
        let JsonIndex { bing, google } = index;

        let JsonBingIndex { key, key_location } = bing;
        let bing = BingIndex::new(
            key.into(),
            key_location
                .map(TryInto::try_into)
                .transpose()
                .map_err(IndexError::KeyLocation)?,
        );

        let JsonGoogleIndex {
            private_key,
            client_email,
            token_uri,
        } = google;
        let google = GoogleIndex::new(private_key.into(), client_email.into(), token_uri.into())
            .map_err(IndexError::GoogleIndex)?;

        Ok(Self { bing, google })
    }
}

impl Indexer {
    pub async fn updated(&self, url: Url) -> Result<(), IndexError> {
        let bing = self
            .bing
            .index_now(&url)
            .await
            .map_err(IndexError::BingIndex);

        let google = self
            .google
            .url_updated(url)
            .await
            .map_err(IndexError::GoogleIndex);

        if bing.is_err() { bing } else { google }
    }

    pub async fn deleted(&self, url: Url) -> Result<(), IndexError> {
        let bing = self
            .bing
            .index_now(&url)
            .await
            .map_err(IndexError::BingIndex);

        let google = self
            .google
            .url_deleted(url)
            .await
            .map_err(IndexError::GoogleIndex);

        if bing.is_err() { bing } else { google }
    }
}
