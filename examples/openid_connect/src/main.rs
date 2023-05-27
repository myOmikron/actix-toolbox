use actix_session::{Session, SessionMiddleware};
use actix_toolbox::oidc::openidconnect::{ClientId, IssuerUrl, RedirectUrl};
use actix_toolbox::oidc::{finish_login, login, Config, Provider, SessionKeys, UserData};
use actix_web::cookie::Key;
use actix_web::http::header;
use actix_web::web::get;
use actix_web::{App, HttpResponse, HttpServer};

use crate::session::MemorySession;

/// Naive in-memory implementation for actix_session
///
/// Use a proper one!
mod session;

async fn index(session: Session) -> HttpResponse {
    let user: Option<UserData> = session
        .get(&SessionKeys::default().data)
        .expect("corrupt session");

    if let Some(user) = user {
        HttpResponse::Ok().body(format!("{claims:#?}", claims = user.claims))
    } else {
        // If the user isn't logged in, redirect him to the login endpoint
        HttpResponse::TemporaryRedirect()
            .append_header((header::LOCATION, "/login"))
            .finish()
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // You probably want to deserialize this struct from a config file
    let config = Config {
        // The url, the `finish_login` handler is exposed under (see below)
        finish_login_url: RedirectUrl::new("/finish_login".into()).expect("Invalid url"),

        // Any url to redirect to once the whole openid connect workflow has finished
        post_auth_url: "/".to_string(),

        // Don't forget to fill in your openid connect provider's details !!!
        provider: Provider {
            client_id: ClientId::new("<your client id>".into()),
            client_secret: None, // You'll probably have a secret
            discover_url: IssuerUrl::new("<your provider's url>".into()).expect("Invalid url"),
        },

        scopes: Default::default(),
        session_keys: Default::default(),
    };

    let client = config.discover().await.expect("Failed openid discover");

    let key = Key::generate();
    HttpServer::new(move || {
        App::new()
            // Setup actix-session
            .wrap(
                SessionMiddleware::builder(MemorySession::default(), key.clone())
                    .cookie_name("session".to_string())
                    .build(),
            )
            // Pass the oidc client to the login and finish_login handler
            .app_data(client.clone())
            // Add the toolbox' login and finish_login handler
            .route("/login", get().to(login))
            .route("/finish_login", get().to(finish_login))
            .route("/", get().to(index))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
