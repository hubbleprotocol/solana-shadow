use std::time::Duration;

use tokio::net::TcpStream;
use tokio_tungstenite::{
  tungstenite::{self, client::IntoClientRequest, handshake::client::Response},
  MaybeTlsStream, WebSocketStream,
};
use tracing_futures::Instrument;
use tungstenite::error::Error;

#[tracing::instrument]
pub async fn connect_async<R>(
  timeout: Duration,
  request: R,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
where
  R: std::fmt::Debug + IntoClientRequest + Unpin + Clone,
{
  let mut delay = 500;
  loop {
    match tokio::time::timeout(
      timeout,
      tokio_tungstenite::connect_async(request.clone())
        .instrument(tracing::info_span!("tungstenite_connect_async")),
    )
    .await
    {
      Err(e) => {
        tracing::warn!(
          error=?e, "timeout while trying to connect to websocket, retrying in {}", delay
        );
      }
      Ok(result) => match result {
        Ok(r) => {
          return Ok(r);
        }
        Err(e) => {
          tracing::warn!(error=?e, "websocket connection error, retrying in {}", delay);
        }
      },
    }
    tokio::time::sleep(Duration::from_millis(delay)).await;
    delay = std::cmp::min(8000, delay * 2);
  }
}
