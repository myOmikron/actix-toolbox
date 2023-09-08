use actix_session::{Session, SessionInsertError};
use actix_web::http::header;
use actix_web::web::{Data, Query, Redirect};
use actix_web::{HttpResponse, ResponseError};
use openidconnect::core::{CoreAuthenticationFlow, CoreRequestTokenError};
use openidconnect::reqwest::{async_http_client, HttpClientError};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClaimsVerificationError, CsrfToken, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, SigningError, TokenResponse,
};
use serde::{Deserialize, Serialize};

use crate::oidc::{Client, UserData};

/// Handler for OIDC's login endpoint
pub async fn login(client: Data<Client>, session: Session) -> Result<Redirect, SessionInsertError> {
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the authorization URL to which we'll redirect the user.
    let mut request = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_pkce_challenge(pkce_code_challenge);
    for scope in &client.scopes {
        request = request.add_scope(scope.clone());
    }
    let (auth_url, csrf_token, nonce) = request.url();

    // Store the csrf_token to verify it in finish_login
    session.insert(
        &client.session_keys.request,
        AuthState {
            csrf_token,
            pkce_code_verifier,
            nonce,
        },
    )?;

    Ok(Redirect::to(auth_url.to_string()).temporary())
}

#[derive(Serialize, Deserialize)]
struct AuthState {
    csrf_token: CsrfToken,
    pkce_code_verifier: PkceCodeVerifier,
    nonce: Nonce,
}

#[derive(Deserialize)]
pub struct AuthRequest {
    code: AuthorizationCode,
    state: CsrfToken,
}

/// Handler for the OIDC endpoint the user will be redirected to from the OIDC provider
pub async fn finish_login(
    client: Data<Client>,
    params: Query<AuthRequest>,
    session: Session,
) -> Result<HttpResponse, FinishLoginError> {
    let AuthRequest { code, state } = params.into_inner();

    // Get and remove the state generated in login
    let AuthState {
        csrf_token,
        pkce_code_verifier,
        nonce,
    } = session
        .remove_as(&client.session_keys.request)
        .ok_or(FinishLoginError::MissingState)?
        .map_err(|_| FinishLoginError::MissingState)?;

    // Check the states to match
    if state.secret() != csrf_token.secret() {
        return Err(FinishLoginError::InvalidState);
    }

    // Exchange the code with a token.
    let token = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(FinishLoginError::FailedRequestToken)?;

    // Extract the ID token claims after verifying its authenticity and nonce.
    let id_token = token.id_token().ok_or(FinishLoginError::MissingIdToken)?;
    let claims = id_token
        .claims(&client.id_token_verifier(), &nonce)
        .map_err(FinishLoginError::InvalidIdToken)?;

    // Verify the access token hash to ensure that the access token hasn't been substituted for
    // another user's.
    if let Some(expected_access_token_hash) = claims.access_token_hash() {
        let actual_access_token_hash = AccessTokenHash::from_token(
            token.access_token(),
            &id_token
                .signing_alg()
                .map_err(FinishLoginError::CreateAccessTokenHash)?,
        )
        .map_err(FinishLoginError::CreateAccessTokenHash)?;
        if actual_access_token_hash != *expected_access_token_hash {
            return Err(FinishLoginError::InvalidAccessTokenHash);
        }
    }

    // Store in session
    session
        .insert(
            &client.session_keys.data,
            UserData {
                claims: claims.clone(),
                token,
            },
        )
        .map_err(FinishLoginError::SessionInsert)?;

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, client.post_auth_url.as_str()))
        .finish())
}

#[derive(Debug)]
pub enum FinishLoginError {
    /// There is no `state` in the user's session
    /// Maybe he hasn't visited [`login`] yet?
    MissingState,

    /// The `state` in the user's session doesn't match the `state` the oidc provider responded with.
    InvalidState,

    /// Failed to request the actual token from the oidc provider
    FailedRequestToken(CoreRequestTokenError<HttpClientError>),

    /// The provider didn't send a id token
    MissingIdToken,

    /// Failed to verify the id token while reading claims
    InvalidIdToken(ClaimsVerificationError),

    /// Error occurring while generating an [`AccessTokenHash`]
    CreateAccessTokenHash(SigningError),

    /// The claims' access token doesn't match the oidc's
    InvalidAccessTokenHash,

    /// Error from [`Session::insert`]
    SessionInsert(SessionInsertError),
}
impl std::fmt::Display for FinishLoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FinishLoginError::MissingState => write!(f, "State is missing from user session"),
            FinishLoginError::InvalidState => write!(f, "State in user session is invalid"),
            FinishLoginError::FailedRequestToken(err) => {
                write!(f, "Failed to request token: {err}")
            }
            FinishLoginError::MissingIdToken => {
                write!(f, "Provider didn't respond with an ID token")
            }
            FinishLoginError::InvalidIdToken(err) => {
                write!(f, "The ID token didn't pass the verification: {err}")
            }
            FinishLoginError::CreateAccessTokenHash(err) => {
                write!(f, "Couldn't generate the access token's hash: {err}")
            }
            FinishLoginError::InvalidAccessTokenHash => {
                write!(f, "The access token's hash doesn't match")
            }
            FinishLoginError::SessionInsert(err) => {
                write!(f, "Failed to set token in user session: {err}")
            }
        }
    }
}
impl std::error::Error for FinishLoginError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FinishLoginError::MissingState => None,
            FinishLoginError::InvalidState => None,
            FinishLoginError::FailedRequestToken(err) => Some(err),
            FinishLoginError::SessionInsert(err) => Some(err),
            FinishLoginError::MissingIdToken => None,
            FinishLoginError::CreateAccessTokenHash(err) => Some(err),
            FinishLoginError::InvalidAccessTokenHash => None,
            FinishLoginError::InvalidIdToken(err) => Some(err),
        }
    }
}
impl ResponseError for FinishLoginError {}
