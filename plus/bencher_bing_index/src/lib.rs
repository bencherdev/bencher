use url::Url;

#[derive(Debug)]
pub struct BingIndex {
    key: String,
    key_location: Option<Url>,
}

#[derive(Debug, thiserror::Error)]
pub enum BingIndexError {
    #[error("Failed to parse IndexNow URL: {0}")]
    ParseUrl(url::ParseError),
    #[error("Failed to send IndexNow request: {0}")]
    BadRequest(reqwest::Error),
    #[error("Bad IndexNow response: {0:?}")]
    BadResponse(reqwest::Response),
}

impl BingIndex {
    pub fn new(key: String, key_location: Option<Url>) -> Self {
        Self { key, key_location }
    }

    pub async fn index_now(&self, url: &Url) -> Result<(), BingIndexError> {
        let url = self.index_now_url(url)?;
        let response = reqwest::get(url)
            .await
            .map_err(BingIndexError::BadRequest)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(BingIndexError::BadResponse(response))
        }
    }

    // https://www.bing.com/indexnow?url=http://www.example.com/product.html&key=bbf489cef4c24ba2b54c991e8b97a1c8
    // https://<searchengine>/indexnow?url=http://www.example.com/product.html&key=2890457b6c8944748b45a3adc885d237&keyLocation=http://www.example.com/myIndexNowKey63638.txt
    fn index_now_url(&self, url: &Url) -> Result<Url, BingIndexError> {
        let mut params = vec![("url", url.as_str()), ("key", &self.key)];
        if let Some(key_location) = &self.key_location {
            params.push(("keyLocation", key_location.as_str()));
        }
        Url::parse_with_params("https://www.bing.com/indexnow", params)
            .map_err(BingIndexError::ParseUrl)
    }
}
