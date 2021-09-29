use crate::{Error, Network, Result};
use dashmap::DashMap;
use futures::{
  stream::{SplitSink, SplitStream},
  SinkExt, StreamExt,
};
use serde_json::json;
use solana_client::client_error::reqwest::Url;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpStream;
use tokio_tungstenite::{
  connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, trace, warn};

type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

#[derive(Debug)]
pub(crate) struct ProgramChange;

#[derive(Debug)]
pub(crate) enum SolanaChange {
  Account((Pubkey, Account)),
  ProgramChange(ProgramChange),
}

enum SolanaMessage {
  ConfirmSubmscription {
    reqid: usize,
    subscription: u64
  },
  AccountNotification {
    subscription: u64,
    account: Account
  }
}

pub(crate) struct SolanaChangeListener {
  reader: WsReader,
  writer: WsWriter,
  reqid: AtomicUsize,
  pending: DashMap<usize, Pubkey>,
  subscriptions: DashMap<u64, Pubkey>,
}

impl SolanaChangeListener {
  pub async fn new(network: Network) -> Result<Self> {
    let mut url: Url = network
      .rpc_url()
      .parse()
      .map_err(|_| Error::InvalidArguemt)?;

    match url.scheme() {
      "http" => url.set_scheme("ws").unwrap(),
      "https" => url.set_scheme("wss").unwrap(),
      _ => panic!("unsupported cluster url scheme"),
    };

    let (ws_stream, _) = connect_async(url).await?;
    let (writer, reader) = ws_stream.split();
    let reqid = AtomicUsize::new(1);
    Ok(Self {
      reader,
      writer,
      reqid,
      pending: DashMap::new(),
      subscriptions: DashMap::new(),
    })
  }

  /// Send a subscription request to Solana Cluster.
  ///
  /// When this method returns, it does not mean that a subscription
  /// has been successfully created, but only the the request was
  /// sent successfully.
  pub async fn subscribe(&mut self, account: Pubkey) -> Result<()> {
    let reqid = self.reqid.fetch_add(1, Ordering::SeqCst);
    trace!("Subscribing to {} using request id {}", &account, reqid);
    let request = Self::account_subscription_request(&account, reqid);

    // map jsonrpc request id to pubkey, later on when
    // the websocket responds with a subscription id,
    // the id will be correlated with a public key.
    // account change notifications don't mention
    // the account public key, instead they use the
    // solana-generated subscription id to identify
    // an account.
    self.pending.insert(reqid, account.clone());
    self.writer.send(Message::Text(request)).await?;
    Ok(())
  }

  pub async fn recv(&mut self) -> Result<Option<SolanaChange>> {
    while let Some(msg) = self.reader.next().await {
      match msg {
        Ok(msg) => self.process_solana_message(msg)?,
        Err(e) => warn!("received ws error from solana: {:?}", &e),
      };
    }

    Ok(None)
  }

  fn process_solana_message(&self, msg: Message) -> Result<()> {
    debug!("[todo] Processing Solana message: {:?}", msg);

    Ok(())
  }

  fn account_subscription_request(account: &Pubkey, id: usize) -> String {
    json!({
      "jsonrpc": "2.0",
      "id": id,
      "method": "accountSubscribe",
      "params": [account.to_string(), {
        "encoding": "jsonParsed",
        "commitment": "finalized"
      }]
    })
    .to_string()
  }
}
