#![allow(clippy::multiple_inherent_impl)]

use bencher_valid::{NonEmpty, Secret};
use oauth2::{ClientId, ClientSecret, RedirectUrl};
use url::Url;

mod auth;

#[derive(Debug, Clone)]
pub struct GitHub {
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect_url: RedirectUrl,
}

impl GitHub {
    pub fn new(endpoint: &Url, client_id: NonEmpty, client_secret: Secret) -> Self {
        let client_id = ClientId::new(client_id.into());
        let client_secret = ClientSecret::new(client_secret.into());
        let mut endpoint = endpoint.clone();
        endpoint.set_path("/auth/github");
        let redirect_url = RedirectUrl::from_url(endpoint);
        Self {
            client_id,
            client_secret,
            redirect_url,
        }
    }
}
