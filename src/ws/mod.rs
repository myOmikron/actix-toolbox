use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use actix::MailboxError;
use actix::{Actor, ActorContext, ActorFuture, Addr, AsyncContext, Handler, StreamHandler};
use actix_web::error::{Error, PayloadError};
use actix_web::web::Bytes;
use actix_web::{HttpRequest, HttpResponse};
pub use actix_web_actors::ws::{Message, ProtocolError};
use actix_web_actors::ws::{WebsocketContext, WsResponseBuilder};
use futures::Stream;
use tokio::sync::mpsc;

/// Perform websocket handshake and produce a [sender](Sender) and [receiver](Receiver) to communicate with the websocket.
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
pub fn start<S>(request: &HttpRequest, stream: S) -> Result<(Sender, Receiver, HttpResponse), Error>
where
    S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
{
    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER);
    WsResponseBuilder::new(WebSocketActor { channel: sender }, request, stream)
        .start_with_addr()
        .map(move |(addr, response)| (Sender { addr }, Receiver { channel: receiver }, response))
}

/// Receiving part of a websocket
///
/// Not cloneable
#[derive(Debug)]
pub struct Receiver {
    channel: mpsc::Receiver<Result<Message, ProtocolError>>,
}
impl Receiver {
    /// Listen to websocket messages.
    ///
    /// - Returns `None` if the websocket was closed.
    /// - Returns `Some(Err(...))` if an invalid websocket frame was received.
    ///
    /// Recommended usage:
    /// ```no_run
    /// # use actix_toolbox::ws;
    /// # fn somewhere() -> ! {panic!();}
    ///
    /// // See ws::start for how to get this struct
    /// let mut receiver: ws::Receiver = somewhere();
    /// tokio::spawn(async move {
    ///     while let Some(message) = receiver.recv().await {
    ///         // Handle incoming message
    ///     }
    ///     // The websocket was closed
    /// });
    /// ```
    ///
    /// For more details see [tokio](mpsc::Receiver::recv).
    pub async fn recv(&mut self) -> Option<Result<Message, ProtocolError>> {
        self.channel.recv().await
    }
}

/// Sending part of a websocket
///
/// Cloneable
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct Sender {
    addr: Addr<WebSocketActor>,
}
impl Sender {
    /// Send a message over the websocket.
    ///
    /// - Returns `Err(...)` if the websocket was closed.
    pub async fn send(&self, msg: Message) -> Result<(), MailboxError> {
        self.addr.send(WrappedMessage::Send(msg)).await
    }

    /// Close the websocket
    ///
    /// - Returns `Err(...)` if the websocket was already closed.
    pub async fn close(&self) -> Result<(), MailboxError> {
        self.addr.send(WrappedMessage::Close).await
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
enum WrappedMessage {
    Send(Message),
    Close,
}
impl actix::Message for WrappedMessage {
    type Result = ();
}

struct WebSocketActor {
    channel: mpsc::Sender<Result<Message, ProtocolError>>,
}

impl Actor for WebSocketActor {
    type Context = WebsocketContext<Self>;
}

impl Handler<WrappedMessage> for WebSocketActor {
    type Result = ();

    fn handle(&mut self, msg: WrappedMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            WrappedMessage::Send(msg) => ctx.write_raw(msg),
            WrappedMessage::Close => ctx.stop(),
        }
    }
}

impl StreamHandler<Result<Message, ProtocolError>> for WebSocketActor {
    fn handle(&mut self, item: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        let channel = self.channel.clone();
        let future = async move { channel.send(item).await };
        ctx.spawn(SendFuture { future });
    }
}

#[pin_project::pin_project]
struct SendFuture<
    F: Future<Output = Result<(), mpsc::error::SendError<Result<Message, ProtocolError>>>>,
> {
    #[pin]
    future: F,
}
impl<F: Future<Output = Result<(), mpsc::error::SendError<Result<Message, ProtocolError>>>>>
    ActorFuture<WebSocketActor> for SendFuture<F>
{
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        _srv: &mut WebSocketActor,
        ctx: &mut WebsocketContext<WebSocketActor>,
        task: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        match self.project().future.poll(task) {
            Poll::Ready(result) => {
                if result.is_err() {
                    ctx.stop();
                }
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
