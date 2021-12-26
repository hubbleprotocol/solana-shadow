use crate::{
  blockchain::SubRequestCall,
  message::{NotificationParams, NotificationValue, SolanaMessage},
  Error, Result, SyncOptions,
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
  sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
  },
};
use tokio::{
  net::TcpStream,
  sync::{oneshot, RwLock},
};
use tokio_tungstenite::{
  connect_async,
  tungstenite::{self, Message},
  MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info, warn};

use crate::blockchain::AccountsMap;
use crate::rpc;

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
  reader: Option<WsReader>,
  writer: Option<WsWriter>,
  reqid: AtomicU64,
  pending: DashMap<u64, SubRequestCall>,
  subscriptions: DashMap<u64, SubRequest>,
  subs_history: RwLock<Vec<SubRequest>>,
  sync_options: SyncOptions,
  client: rpc::ClientBuilder,
  accounts: Arc<AccountsMap>,
}

impl SolanaChangeListener {
  pub async fn new(
    client: rpc::ClientBuilder,
    accounts: Arc<AccountsMap>,
    sync_options: SyncOptions,
  ) -> Result<Self> {
    let mut url: Url = sync_options
      .network
      .wss_url()
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
      reader: Some(reader),
      writer: Some(writer),
      reqid: AtomicU64::new(1),
      pending: DashMap::new(),
      subscriptions: DashMap::new(),
      subs_history: RwLock::new(Vec::new()),
      sync_options,
      client,
      accounts,
    })
  }

  /// Send an account subscription request to Solana Cluster.
  ///
  /// When this method returns, it does not mean that a subscription
  /// has been successfully created, but only the the request was
  /// sent successfully.
  pub async fn subscribe_account(
    &mut self,
    account: Pubkey,
    oneshot: Option<oneshot::Sender<Vec<Account>>>,
  ) -> Result<()> {
    self
      .subscribe_account_internal(account, oneshot, true)
      .await
  }

  #[tracing::instrument(skip(self, oneshot))]
  async fn subscribe_account_internal(
    &mut self,
    account: Pubkey,
    oneshot: Option<oneshot::Sender<Vec<Account>>>,
    record: bool,
  ) -> Result<()> {
    let reqid = self.reqid.fetch_add(1, Ordering::SeqCst);
    let request = json!({
      "jsonrpc": "2.0",
      "id": reqid,
      "method": "accountSubscribe",
      "params": [account.to_string(), {
        "encoding": "jsonParsed",
        "commitment": self.sync_options.commitment.to_string(),
      }]
    });

    let sub_request = SubRequest::Account(account);

    if record {
      // keep a copy of this request in the subscriptions
      // history log, so that when a reconnect event occurs
      // all those subscription messages are going to be replayed
      let mut history = self.subs_history.write().await;
      history.push(sub_request);
    }

    // map jsonrpc request id to pubkey, later on when
    // the websocket responds with a subscription id,
    // the id will be correlated with a public key.
    // account change notifications don't mention
    // the account public key, instead they use the
    // solana-generated subscription id to identify
    // an account.
    self.pending.insert(reqid, (sub_request, oneshot));
    loop {
      if let Some(ref mut writer) = self.writer {
        debug!(request=%request, "accountSubscribe send over websocket");
        writer.send(Message::Text(request.to_string())).await?;
        break;
      } else {
        debug!("skipping sending no writer available");
        tokio::task::yield_now().await;
      }
    }
    Ok(())
  }

  /// Send a program subscription request to Solana Cluster.
  ///
  /// When this method returns, it does not mean that a subscription
  /// has been successfully created, but only the the request was
  /// sent successfully.
  pub async fn subscribe_program(
    &mut self,
    account: Pubkey,
    oneshot: Option<oneshot::Sender<Vec<Account>>>,
  ) -> Result<()> {
    self
      .subscribe_program_internal(account, oneshot, true)
      .await
  }

  #[tracing::instrument(skip(self, oneshot))]
  async fn subscribe_program_internal(
    &mut self,
    account: Pubkey,
    oneshot: Option<oneshot::Sender<Vec<Account>>>,
    record: bool,
  ) -> Result<()> {
    let reqid = self.reqid.fetch_add(1, Ordering::SeqCst);
    let request = json!({
      "jsonrpc": "2.0",
      "id": reqid,
      "method": "programSubscribe",
      "params": [account.to_string(), {
        "encoding": "jsonParsed",
        "commitment": self.sync_options.commitment.to_string()
      }]
    });

    let sub_request = SubRequest::Program(account);

    if record {
      // keep a copy of this request in the subscriptions
      // history log, so that when a reconnect event occurs
      // all those subscription messages are going to be replayed
      let mut history = self.subs_history.write().await;
      history.push(sub_request);
    }

    // map jsonrpc request id to pubkey, later on when
    // the websocket responds with a subscription id,
    // the id will be correlated with a public key.
    // account change notifications don't mention
    // the account public key, instead they use the
    // solana-generated subscription id to identify
    // an account.
    self.pending.insert(reqid, (sub_request, oneshot));
    loop {
      if let Some(ref mut writer) = self.writer {
        debug!(request=%request, "programSubscribe send over websocket");
        writer.send(Message::Text(request.to_string())).await?;
        break;
      } else {
        debug!("skipping sending no writer available");
        tokio::task::yield_now().await;
      }
    }
    Ok(())
  }

  #[tracing::instrument(skip(self))]
  pub async fn recv(&mut self) -> Result<Option<AccountUpdate>> {
    loop {
      if let Some(ref mut reader) = self.reader {
        while let Some(msg) = reader.next().await {
          let message = match msg {
            Ok(msg) => match msg {
              Message::Text(text) => Ok(serde_json::from_str(&text)?),
              _ => Err(Error::UnsupportedRpcFormat),
            },
            Err(e) => {
              warn!("received ws error from solana: {:?}", &e);
              Err(Error::WebSocketError(e))
            }
          }?;

          tracing::trace!(?message, "received from wss");

          // This message is a JSON-RPC response to a subscription request.
          // Here we are mapping the request id with the subscription id,
          // and creating a map of subscription id => pubkey.
          // This type of message is not relevant to the external callers
          // of this method, so we keep looping and listening for interesting
          // notifications.
          use dashmap::mapref::entry::Entry::{Occupied, Vacant};
          if let SolanaMessage::Confirmation { id, result, .. } = message {
            if let Some((_, (sub_request, oneshot))) = self.pending.remove(&id)
            {
              self.subscriptions.insert(result, sub_request);
              match sub_request {
                SubRequest::Account(account) => {
                  let (key, acc) =
                    rpc::get_account(self.client.clone(), account).await?;

                  // only insert entry if we have not received an update
                  // from our subscription
                  match self.accounts.entry(key) {
                    Occupied(mut e) => {
                      let (_, updated) = e.get().clone();

                      tracing::debug!(?account, ?updated, "occupied branch");
                      if !updated {
                        // we update with our RPC values
                        e.insert((acc.clone(), true));
                      }
                    }
                    Vacant(e) => {
                      tracing::debug!(?account, "vacant branch");
                      e.insert((acc, true));
                    }
                  }

                  if let Some(oneshot) = oneshot {
                    let account = self.accounts.get(&key).unwrap().0.clone();

                    tracing::debug!(?account, "oneshot send");
                    if oneshot.send(vec![account]).is_err() {
                      tracing::warn!("receiver dropped")
                    }
                  }
                }
                SubRequest::Program(program_id) => {
                  // we have a successful program subscription, so now lets get
                  // all program accounts and add them to our accounts
                  let accounts: Vec<_> =
                    rpc::get_program_accounts(self.client.clone(), &program_id)
                      .await?;

                  // only insert entry if we have not received an update
                  // from our subscription
                  let mut result: Vec<Account> = vec![];
                  for (key, acc) in accounts {
                    match self.accounts.entry(key) {
                      Occupied(mut e) => {
                        let (account, updated) = e.get().clone();

                        tracing::debug!(
                          ?program_id,
                          ?account,
                          ?updated,
                          "occupied branch"
                        );
                        if !updated {
                          // we update with our RPC values
                          e.insert((acc.clone(), true));
                          result.push(acc);
                        } else {
                          // value updated over ws subscription
                          // no need to update, and we push
                          // the ws value to results to return the
                          // latest
                          result.push(account);
                        }
                      }
                      Vacant(e) => {
                        tracing::debug!(?program_id, ?acc, "vacant branch");
                        e.insert((acc, true));
                      }
                    }
                  }

                  // return value all the way back to the caller
                  if let Some(oneshot) = oneshot {
                    tracing::debug!(?program_id, accounts_len=?result.len(), "oneshot send");
                    if oneshot.send(result).is_err() {
                      tracing::warn!("receiver dropped")
                    }
                  }
                }
                SubRequest::ReconnectAll => {
                  // note: we safely ignore this one
                }
              };

              debug!("created subscripton {} for {:?}", &result, &sub_request);
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
      } else {
        tokio::task::yield_now().await;
      }
    }
  }

  pub async fn reconnect_all(&mut self) -> Result<()> {
    let old_reader = self.reader.take();
    let old_writer = self.writer.take();

    let (ws_stream, _) = connect_async(self.url.clone()).await?;
    let (writer, reader) = ws_stream.split();

    self.reader = Some(reader);
    self.writer = Some(writer);
    self.pending = DashMap::new();
    self.subscriptions = DashMap::new();

    // mark all values in account as cleared
    self.accounts.alter_all(|_, (account, _)| (account, false));

    let mut stream = old_writer
      .unwrap()
      .reunite(old_reader.unwrap())
      .map_err(|_| Error::InternalError)?;
    stream.close(None).await.unwrap_or_else(|e| {
      if let tungstenite::Error::AlreadyClosed = e {
        // this is expected.
        debug!("Connection to solana closed");
      } else {
        // leave a trace in the log for easier debugging
        warn!("failed closing connection to Solana: {:?}", e);
      }
    });

    let history = {
      let shared_history = self.subs_history.read().await;
      shared_history.clone()
    };

    for sub in history.iter() {
      match sub {
        SubRequest::Account(acc) => {
          info!("recreating account subscription for {}", &acc);
          self.subscribe_account_internal(*acc, None, false).await?
        }
        SubRequest::Program(acc) => {
          info!("recreating program subscription for {}", &acc);
          self.subscribe_program_internal(*acc, None, false).await?
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
        if let Some(SubRequest::Account(pubkey)) =
          self.subscriptions.get(&params.subscription).as_deref()
        {
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
}
