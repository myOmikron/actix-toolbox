mod config;
mod handler;

/// Re-export the wrapped Open ID Connect implementation
pub use openidconnect;
use openidconnect::core::{CoreIdTokenClaims, CoreTokenResponse};
use serde::{Deserialize, Serialize};

pub use crate::oidc::config::{Client, Config, Provider, SessionKeys};
pub use crate::oidc::handler::{finish_login, login};

/// Data the [`finish_login`] handler will store in the user's session
#[derive(Serialize, Deserialize)]
pub struct UserData {
    /// The oidc token
    pub token: CoreTokenResponse,

    /// The OIDC claims
    pub claims: CoreIdTokenClaims,
}
