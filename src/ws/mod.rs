use actix::{Actor, Addr, AsyncContext, Handler, MailboxError, Message, StreamHandler, WrapFuture};
use actix_web::error::{Error, PayloadError};
use actix_web::web::Bytes;
use actix_web::{HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures::Stream;
use tokio::sync::mpsc::{channel, Receiver, Sender};

pub use actix_web_actors::ws::Message as WebSocketMessage;

/// Perform websocket handshake and produce a [sender](WebSocketSender) and [receiver](WebSocketReceiver) to communicate with the websocket.
///
/// ```no_run
/// use actix_web::{HttpRequest, HttpResponse};
/// use actix_web::web::Payload;
/// use actix_web::error::Error;
///
/// use actix_toolbox::ws;
///
/// async fn request_handler(request: HttpRequest, payload: Payload) -> Result<HttpResponse, Error> {
///     let (sender, mut receiver, response) = ws::start(&request, payload)?;
///     
///     // Spawn tasks using the sender and receiver here
///
///     Ok(response)
/// }
/// ```
pub fn start<S>(
    request: &HttpRequest,
    stream: S,
) -> Result<(WebSocketSender, WebSocketReceiver, HttpResponse), Error>
where
    S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
{
    let (sender, receiver) = channel(CHANNEL_BUFFER);
    ws::WsResponseBuilder::new(WebSocketActor { channel: sender }, request, stream)
        .start_with_addr()
        .map(move |(addr, response)| {
            (
                WebSocketSender { addr },
                WebSocketReceiver { channel: receiver },
                response,
            )
        })
}

/// Receiving part of a websocket
///
/// Not cloneable
#[derive(Debug)]
pub struct WebSocketReceiver {
    channel: Receiver<WebSocketMessage>,
}
impl WebSocketReceiver {
    /// See [tokio](Receiver::recv)
    pub async fn recv(&mut self) -> Option<WebSocketMessage> {
        self.channel.recv().await
    }
}

/// Sending part of a websocket
///
/// Cloneable
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct WebSocketSender {
    addr: Addr<WebSocketActor>,
}
impl WebSocketSender {
    /// See [actix](Addr::send)
    pub async fn send(&self, msg: WebSocketMessage) -> Result<(), MailboxError> {
        self.addr.send(WrappedMessage(msg)).await
    }
}

/// Buffer size for the "rust -> websocket" channel.
///
/// The other direction uses actix internal mailbox.
///
/// This constant was found and copied from deep inside of actix,
/// namely the [Default] impl for [Mailbox](actix::dev::Mailbox) which is used by [ws::start]
pub const CHANNEL_BUFFER: usize = 16;

#[derive(Debug, Eq, PartialEq)]
struct WrappedMessage(WebSocketMessage);

impl WebSocketSender {}

struct WebSocketActor {
    channel: Sender<WebSocketMessage>,
}

impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;
}

impl Message for WrappedMessage {
    type Result = ();
}
impl Handler<WrappedMessage> for WebSocketActor {
    type Result = ();

    fn handle(&mut self, msg: WrappedMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.write_raw(msg.0);
    }
}

impl StreamHandler<Result<WebSocketMessage, ws::ProtocolError>> for WebSocketActor {
    fn handle(
        &mut self,
        item: Result<WebSocketMessage, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match item {
            Ok(msg) => {
                let channel = self.channel.clone();
                ctx.spawn(async move { channel.send(msg).await.expect("TODO") }.into_actor(&*self));
            }
            Err(_) => unimplemented!(),
        }
    }
}
