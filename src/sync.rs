use crate::{
  message::{NotificationParams, NotificationValue, SolanaMessage},
  Error, Network, Result,
};
use dashmap::DashMap;
use futures::{
  stream::{SplitSink, SplitStream},
  SinkExt, StreamExt,
};
use serde_json::json;
use solana_client::client_error::reqwest::Url;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::{
  convert::TryInto,
  sync::atomic::{AtomicU64, Ordering},
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
  connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, warn};

type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

#[derive(Debug)]
pub(crate) struct ProgramChange;

/// Changes communicated by the solana cluster
/// to which we are subscribed.
#[derive(Debug)]
pub(crate) enum SolanaChange {
  /// changes to the contents to one account.
  Account((Pubkey, Account)),

  /// changes to a program
  ///
  /// This includes new owned accounts, etc, so whenever
  /// a program creates a new account it gets detected
  /// automatically and an account subscription is created
  /// for it.
  ProgramChange(ProgramChange),
}

pub(crate) struct SolanaChangeListener {
  reader: WsReader,
  writer: WsWriter,
  reqid: AtomicU64,
  pending: DashMap<u64, Pubkey>,
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
    Ok(Self {
      reader,
      writer,
      reqid: AtomicU64::new(1),
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
      let message = match msg {
        Ok(msg) => self.decode_message(msg),
        Err(e) => {
          warn!("received ws error from solana: {:?}", &e);
          Err(Error::WebSocketError(e))
        }
      }?;
      // This message is a JSON-RPC response to a subscription request.
      // Here we are mapping the request id with the subscription id,
      // and creating a map of subscription id => pubkey.
      // This type of message is not relevant to the external callers
      // of this method, so we keep looping and listening for interesting
      // notifications.
      if let SolanaMessage::Confirmation { id, result, .. } = message {
        if let Some(pubkey) = self.pending.get(&id) {
          self.subscriptions.insert(result, *pubkey); // todo remove from pending
          debug!("created subscripton {} for {}", &result, &*pubkey);
        } else {
          warn!("Unrecognized subscription id: ({}, {})", id, result);
        }
      }

      // This is a notification call from Solana telling us to either an
      // account or a program has changed.
      if let SolanaMessage::Notification { method, params, .. } = message {
        match &method[..] {
          "accountNotification" => {
            return Ok(Some(self.notification_to_change(params)?));
          }
          _ => {
            // todo program updates
            warn!("unrecognized notification type: {}", &method);
          }
        }
      }
    }

    Ok(None)
  }

  fn notification_to_change(
    &self,
    params: NotificationParams,
  ) -> Result<SolanaChange> {
    if let Some(pubkey) = self.subscriptions.get(&params.subscription) {
      match params.result.value {
        NotificationValue::Account(acc) => {
          Ok(SolanaChange::Account((*pubkey, acc.try_into()?)))
        }
        NotificationValue::Program(_) => {
          Ok(SolanaChange::ProgramChange(ProgramChange))
        }
      }
    } else {
      warn!("Unknown subscription: {}", &params.subscription);
      Err(Error::UnknownSubscription)
    }
  }

  fn decode_message(&self, msg: Message) -> Result<SolanaMessage> {
    match msg {
      Message::Text(text) => Ok(serde_json::from_str(&text)?),
      _ => Err(Error::UnsupportedRpcFormat),
    }
  }

  fn account_subscription_request(account: &Pubkey, id: u64) -> String {
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
