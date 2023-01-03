//! This example starts an actix server accepting websockets at "/" which echos back any message.
use actix_toolbox::ws;
use actix_web::web::Payload;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use log::info;

async fn ws(request: HttpRequest, payload: Payload) -> Result<HttpResponse, Error> {
    let (sender, mut receiver, response) = ws::start(&request, payload)?;
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
        info!("Terminating echo task...");
    });
    Ok(response)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().route("/", web::get().to(ws)))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
