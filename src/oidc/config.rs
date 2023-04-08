use std::collections::HashSet;
use std::ops::Deref;

use actix_web::web::Data;
use openidconnect::core::{CoreClient, CoreProviderMetadata};
use openidconnect::reqwest::{async_http_client, HttpClientError};
use openidconnect::{ClientId, ClientSecret, DiscoveryError, IssuerUrl, RedirectUrl, Scope};
use serde::{Deserialize, Serialize};

/// Configuration for Open ID Connect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Url to [`finish_login`]
    pub finish_login_url: RedirectUrl,

    /// Url [`finish_login`] will redirect to
    pub post_auth_url: String,

    /// Data about the oidc provider
    pub provider: Provider,

    /// List of scopes to request from oidc provider
    pub scopes: HashSet<Scope>,

    /// Set of keys (strings) under which this modules stores its data in the user's session
    ///
    /// Provides a [`Default::default`]
    pub session_keys: SessionKeys,
}

/// Set of keys (strings) under which this modules stores its data in the user's session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionKeys {
    /// Key to store the data required for a secure OIDC request
    ///
    /// I.e. csrf token, some nonce, etc.
    pub request: String,

    /// Key to store the resulting [`UserData`](crate::oidc::UserData)
    pub data: String,
}
impl Default for SessionKeys {
    fn default() -> Self {
        Self {
            request: String::from("oidc_request"),
            data: String::from("oidc_data"),
        }
    }
}

/// Data about the oidc provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// The id your application is registered as with the oidc provider
    pub client_id: ClientId,

    /// The secret your application uses for the oidc provider
    pub client_secret: Option<ClientSecret>,

    /// The oidc provider's auth url
    pub discover_url: IssuerUrl,
}

impl Config {
    /// Fetch the provider's metadata using discovery and create a client
    ///
    /// The [`Ok`] value should be passed to [`App::app_data`](actix_web::App::app_data)
    pub async fn discover(self) -> Result<Data<Client>, DiscoveryError<HttpClientError>> {
        let Config {
            finish_login_url,
            post_auth_url,
            provider:
                Provider {
                    client_id,
                    client_secret,
                    discover_url,
                },
            scopes,
            session_keys,
        } = self;

        let provider_metadata =
            CoreProviderMetadata::discover_async(discover_url, async_http_client).await?;
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, client_secret)
                .set_redirect_uri(finish_login_url);

        Ok(Data::new(Client {
            client,
            post_auth_url,
            scopes,
            session_keys,
        }))
    }
}

/// Client the [`handler`] depend on
pub struct Client {
    pub(crate) client: CoreClient,
    pub(crate) post_auth_url: String,
    pub(crate) scopes: HashSet<Scope>,
    pub(crate) session_keys: SessionKeys,
}

impl Deref for Client {
    type Target = CoreClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}
