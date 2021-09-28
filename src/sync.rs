use solana_client::client_error::reqwest::Url;
use solana_sdk::pubkey::Pubkey;
use tokio::{
  net::TcpStream,
  sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{Error, Network, Result};

type OptionalTlsStream = MaybeTlsStream<TcpStream>;
type WsStream = WebSocketStream<OptionalTlsStream>;

#[derive(Debug)]
pub(crate) struct AccountChange;

#[derive(Debug)]
pub(crate) struct ProgramChange;

#[derive(Debug)]
pub(crate) enum SolanaChange {
  Account(AccountChange),
  ProgramChange(ProgramChange),
}

pub(crate) struct SolanaChangeListener {
  sender: UnboundedSender<SolanaChange>,
  receiver: UnboundedReceiver<SolanaChange>,
}

impl SolanaChangeListener {
  pub fn new(accounts: &[Pubkey], network: Network) -> Result<Self> {
    let mut url: Url = network
      .rpc_url()
      .parse()
      .map_err(|_| Error::InvalidArguemt)?;

    match url.scheme() {
      "http" => url.set_scheme("ws").unwrap(),
      "https" => url.set_scheme("wss").unwrap(),
      _ => panic!("unsupported cluster url scheme"),
    };

    let (tx, rx) = unbounded_channel();
    Ok(Self {
      sender: tx,
      receiver: rx,
    })
  }

  pub async fn recv(&mut self) -> Option<SolanaChange> {
    self.receiver.recv().await
  }
}
