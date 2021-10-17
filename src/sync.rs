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
use tokio::{net::TcpStream, sync::RwLock};
use tokio_tungstenite::{
  connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info, warn};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsReader = SplitStream<WsStream>;
type WsWriter = SplitSink<WsStream, Message>;

#[derive(Debug)]
pub(crate) struct AccountUpdate {
  pub pubkey: Pubkey,
  pub account: Account,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SubRequest {
  Account(Pubkey),
  Program(Pubkey),
  ReconnectAll,
}

pub(crate) struct SolanaChangeListener {
  url: Url,
  reader: WsReader,
  writer: WsWriter,
  reqid: AtomicU64,
  pending: DashMap<u64, Pubkey>,
  subscriptions: DashMap<u64, Pubkey>,
  subs_history: RwLock<Vec<SubRequest>>,
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

    let (ws_stream, _) = connect_async(url.clone()).await?;
    let (writer, reader) = ws_stream.split();
    Ok(Self {
      url,
      reader,
      writer,
      reqid: AtomicU64::new(1),
      pending: DashMap::new(),
      subscriptions: DashMap::new(),
      subs_history: RwLock::new(Vec::new()),
    })
  }

  /// Send an account subscription request to Solana Cluster.
  ///
  /// When this method returns, it does not mean that a subscription
  /// has been successfully created, but only the the request was
  /// sent successfully.
  pub async fn subscribe_account(&mut self, account: Pubkey) -> Result<()> {
    self.subscribe_account_internal(account, true).await
  }

  async fn subscribe_account_internal(
    &mut self,
    account: Pubkey,
    record: bool,
  ) -> Result<()> {
    let reqid = self.reqid.fetch_add(1, Ordering::SeqCst);
    let request = json!({
      "jsonrpc": "2.0",
      "id": reqid,
      "method": "accountSubscribe",
      "params": [account.to_string(), {
        "encoding": "jsonParsed",
        "commitment": "finalized"
      }]
    });

    if record {
      // keep a copy of this request in the subscriptions
      // history log, so that when a reconnect event occurs
      // all those subscription messages are going to be replayed
      let mut history = self.subs_history.write().await;
      history.push(SubRequest::Account(account.clone()));
    }

    // map jsonrpc request id to pubkey, later on when
    // the websocket responds with a subscription id,
    // the id will be correlated with a public key.
    // account change notifications don't mention
    // the account public key, instead they use the
    // solana-generated subscription id to identify
    // an account.
    self.pending.insert(reqid, account.clone());
    self.writer.send(Message::Text(request.to_string())).await?;
    Ok(())
  }

  /// Send a program subscription request to Solana Cluster.
  ///
  /// When this method returns, it does not mean that a subscription
  /// has been successfully created, but only the the request was
  /// sent successfully.
  pub async fn subscribe_program(&mut self, account: Pubkey) -> Result<()> {
    self.subscribe_program_internal(account, true).await
  }

  async fn subscribe_program_internal(
    &mut self,
    account: Pubkey,
    record: bool,
  ) -> Result<()> {
    let reqid = self.reqid.fetch_add(1, Ordering::SeqCst);
    let request = json!({
      "jsonrpc": "2.0",
      "id": reqid,
      "method": "programSubscribe",
      "params": [account.to_string(), {
        "encoding": "jsonParsed",
        "commitment": "finalized"
      }]
    });

    if record {
      // keep a copy of this request in the subscriptions
      // history log, so that when a reconnect event occurs
      // all those subscription messages are going to be replayed
      let mut history = self.subs_history.write().await;
      history.push(SubRequest::Program(account.clone()));
    }

    // map jsonrpc request id to pubkey, later on when
    // the websocket responds with a subscription id,
    // the id will be correlated with a public key.
    // account change notifications don't mention
    // the account public key, instead they use the
    // solana-generated subscription id to identify
    // an account.
    self.pending.insert(reqid, account.clone());
    self.writer.send(Message::Text(request.to_string())).await?;
    Ok(())
  }

  pub async fn recv(&mut self) -> Result<Option<AccountUpdate>> {
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
          "accountNotification" | "programNotification" => {
            return Ok(Some(self.account_notification_to_change(params)?));
          }
          _ => {
            warn!("unrecognized notification type: {}", &method);
          }
        }
      }
    }

    Ok(None)
  }

  pub async fn reconnect_all(&mut self) -> Result<()> {
    self.writer.close().await?;

    let (ws_stream, _) = connect_async(self.url.clone()).await?;
    let (writer, reader) = ws_stream.split();

    self.reader = reader;
    self.writer = writer;
    self.pending = DashMap::new();
    self.subscriptions = DashMap::new();

    #[allow(unused_assignments)]
    let mut history = Vec::new();
    {
      let shared_history = self.subs_history.read().await;
      history = shared_history.clone();
    }

    for sub in history.iter() {
      match sub {
        SubRequest::Account(acc) => {
          info!("recreating account subscription for {}", &acc);
          self.subscribe_account_internal(*acc, false).await?
        }
        SubRequest::Program(acc) => {
          info!("recreating program subscription for {}", &acc);
          self.subscribe_program_internal(*acc, false).await?
        }
        _ => panic!("invalid history value"),
      }
    }

    Ok(())
  }

  fn account_notification_to_change(
    &self,
    params: NotificationParams,
  ) -> Result<AccountUpdate> {
    match params.result.value {
      NotificationValue::Account(acc) => {
        if let Some(pubkey) = self.subscriptions.get(&params.subscription) {
          Ok(AccountUpdate {
            pubkey: *pubkey,
            account: acc.try_into()?,
          })
        } else {
          warn!("Unknown subscription: {}", &params.subscription);
          Err(Error::UnknownSubscription)
        }
      }
      NotificationValue::Program(progacc) => Ok(AccountUpdate {
        pubkey: progacc.pubkey.parse()?,
        account: progacc.account.try_into()?,
      }),
    }
  }

  fn decode_message(&self, msg: Message) -> Result<SolanaMessage> {
    match msg {
      Message::Text(text) => Ok(serde_json::from_str(&text)?),
      _ => Err(Error::UnsupportedRpcFormat),
    }
  }
}
