use crate::{
  rpc,
  sync::{AccountUpdate, SolanaChangeListener, SubRequest},
  Error, Network, Result,
};
use dashmap::DashMap;
use futures::future::try_join_all;
use solana_sdk::{
  account::Account, commitment_config::CommitmentLevel, pubkey::Pubkey,
};
use std::{sync::Arc, time::Duration};
use tokio::{
  sync::{
    broadcast::{self, Receiver, Sender},
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    oneshot,
  },
  task::JoinHandle,
  time::{interval, interval_at, Instant},
};
use tracing::{debug, error};
use tracing_futures::Instrument;

pub(crate) type AccountsMap = DashMap<Pubkey, (Account, bool)>;
pub(crate) type SubRequestCall =
  (SubRequest, Option<oneshot::Sender<Vec<Account>>>);

/// This parameter control how many updates are going to be stored in memory
/// for update receivers (if any subscribed) before it starts returning dropping
/// them and returning RecvError::Lagging.
/// For more info see: https://docs.rs/tokio/1.12.0/tokio/sync/broadcast/index.html#lagging
/// for now it is set to 64, which gives us about 30 seconds on Solana
/// as there can be an update at most once every 400 miliseconds (blocktime)
const MAX_UPDATES_SUBSCRIBER_LAG: usize = 64;
const MIN_RECONNECT_EVERY: u64 = 5;

#[derive(Clone)]
pub struct SyncOptions {
  pub network: Network,
  pub max_lag: Option<usize>,
  pub reconnect_every: Option<Duration>,
  pub rpc_timeout: Duration,
  pub ws_connect_timeout: Duration,
  pub commitment: CommitmentLevel,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      network: Network::Mainnet,
      max_lag: None,
      reconnect_every: None,
      commitment: CommitmentLevel::Finalized,
      rpc_timeout: Duration::from_secs(12),
      ws_connect_timeout: Duration::from_secs(12),
    }
  }
}

/// The entry point to the Solana Blockchain Shadow API
///
/// This type allows its users to monitor several individual
/// accounts or all accounts of a program, or a combination
/// of both for any changes to those accounts and have the
/// most recent version of those accounts available locally
/// and accessible as if they were stored in a local
/// `hashmap<Pubkey, Account>`
pub struct BlockchainShadow {
  options: SyncOptions,
  accounts: Arc<AccountsMap>,
  sub_req: UnboundedSender<SubRequestCall>,
  sync_worker: Option<JoinHandle<Result<()>>>,
  monitor_worker: Option<JoinHandle<Result<()>>>,
  ext_updates: Sender<(Pubkey, Account)>,
}

// public methods
impl BlockchainShadow {
  pub async fn new(options: SyncOptions) -> Result<Self> {
    let max_lag = options.max_lag.unwrap_or(MAX_UPDATES_SUBSCRIBER_LAG);

    // protect our enduser against to quick reconnects which would
    // cause unstable behaviour
    if options
      .reconnect_every
      .map(|v| v < Duration::from_secs(MIN_RECONNECT_EVERY))
      .unwrap_or(false)
    {
      panic!("low reconnect_every duration causes unstable behaviour, minimum reconnect_every value is {} seconds", MIN_RECONNECT_EVERY)
    }

    let (subscribe_tx, subscribe_rx) = unbounded_channel::<SubRequestCall>();

    let mut instance = Self {
      options,
      accounts: Arc::new(AccountsMap::new()),
      sync_worker: None,
      monitor_worker: None,
      sub_req: subscribe_tx,
      ext_updates: broadcast::channel(max_lag).0,
    };

    instance.create_worker(subscribe_rx).await?;
    instance.create_monitor_worker().await?;

    Ok(instance)
  }

  pub async fn add_accounts(
    &mut self,
    accounts: &[Pubkey],
  ) -> Result<Vec<Option<Account>>> {
    let mut results = Vec::new();

    for key in accounts {
      let (oneshot, result) = oneshot::channel::<Vec<Account>>();

      self
        .sub_req
        .clone()
        .send((SubRequest::Account(*key), Some(oneshot)))
        .map_err(|_| Error::InternalError)?;

      results.push(result);
    }
    let results = try_join_all(results).await?;
    let results = results.into_iter().map(|mut v| v.pop()).collect();

    Ok(results)
  }

  pub async fn add_account(
    &mut self,
    account: &Pubkey,
  ) -> Result<Option<Account>> {
    let (oneshot, result) = oneshot::channel::<Vec<Account>>();

    self
      .sub_req
      .clone()
      .send((SubRequest::Account(*account), Some(oneshot)))
      .map_err(|_| Error::InternalError)?;

    Ok(result.await?.pop())
  }

  pub async fn add_program(
    &mut self,
    program_id: &Pubkey,
  ) -> Result<Vec<Account>> {
    let (oneshot, result) = oneshot::channel();

    self
      .sub_req
      .clone()
      .send((SubRequest::Program(*program_id), Some(oneshot)))
      .map_err(|_| Error::InternalError)?;

    Ok(result.await?)
  }

  pub async fn new_for_accounts(
    accounts: &[Pubkey],
    options: SyncOptions,
  ) -> Result<Self> {
    let mut instance = BlockchainShadow::new(options).await?;
    instance.add_accounts(accounts).await?;
    Ok(instance)
  }

  pub async fn new_for_program(
    program: &Pubkey,
    options: SyncOptions,
  ) -> Result<Self> {
    let mut instance = BlockchainShadow::new(options).await?;
    instance.add_program(program).await?;
    Ok(instance)
  }

  pub const fn network(&self) -> &Network {
    &self.options.network
  }

  pub fn len(&self) -> usize {
    self.accounts.len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn for_each_account(&self, mut op: impl FnMut(&Pubkey, &Account)) {
    for pair in self.accounts.iter() {
      let pubkey = pair.pair().0;
      let (account, _) = pair.pair().1;
      op(pubkey, account);
    }
  }

  pub fn get_account(&self, key: &Pubkey) -> Option<Account> {
    self.accounts.get(key).map(|acc| acc.clone().0)
  }

  pub async fn worker(mut self) -> Result<()> {
    match self.sync_worker.take() {
      Some(handle) => Ok(handle.await??),
      None => Err(Error::WorkerDead),
    }
  }

  pub fn updates_channel(&self) -> Receiver<(Pubkey, Account)> {
    self.ext_updates.subscribe()
  }
}

impl BlockchainShadow {
  async fn create_worker(
    &mut self,
    mut subscribe_rx: UnboundedReceiver<SubRequestCall>,
  ) -> Result<()> {
    // subscription requests from blockchain shadow -> listener
    let accs_ref = self.accounts.clone();
    let updates_tx = self.ext_updates.clone();
    let options = self.options.clone();
    let client = rpc::ClientBuilder::new(
      self.network().rpc_url(),
      self.options.rpc_timeout,
      self.options.commitment,
    );
    self.sync_worker = Some(tokio::spawn(
      async move {
        let mut listener =
          SolanaChangeListener::new(client, accs_ref.clone(), options).await?;
        loop {
          tokio::select! {
            message = listener.recv() => {
              let message = match message {
                Ok(m) => m,
                Err(e) => {
                  error!("error in the sync worker thread: {:?}", e);
                  listener.reconnect_all().await?;
                  continue;
                }
              };

              let account_update = listener.process_message(message).await?;
              if let Some(AccountUpdate { pubkey, account }) = account_update {
                debug!("account {} updated", &pubkey);
                accs_ref.insert(pubkey, ( account.clone(), true ));
                if updates_tx.receiver_count() != 0 {
                  updates_tx.send((pubkey, account)).unwrap();
                }
              }
            },
            Some(subreq) = subscribe_rx.recv() => {
              match subreq {
                ( SubRequest::Account(pubkey), oneshot ) => {
                    debug!(?pubkey, "subscribe_account recv");
                    listener.subscribe_account(pubkey, oneshot).await?
                },
                ( SubRequest::Program(pubkey), oneshot ) => {
                    debug!(?pubkey, "subscribe_program recv");
                    listener.subscribe_program(pubkey, oneshot).await?
                },
                ( SubRequest::ReconnectAll, _ ) => {
                    debug!("reconnect_all recv");
                    listener.reconnect_all().await?
                }
                ( SubRequest::Ping, _ ) => {
                  debug!("ping recv");
                  listener.ping().await?
                }
              }
            }
          };
        }
      }
      .instrument(tracing::debug_span!("worker_loop")),
    ));

    Ok(())
  }

  async fn create_monitor_worker(&mut self) -> Result<()> {
    let mut ping_timer = interval(Duration::from_secs(5));
    let reconnect_every = self.options.reconnect_every;

    let channel = self.sub_req.clone();

    let fut = async move {
      if let Some(every) = reconnect_every {
        let mut reconnect_timer = interval_at(Instant::now() + every, every);
        loop {
          let req = tokio::select! {
            _ = ping_timer.tick() => SubRequest::Ping,
            _ = reconnect_timer.tick() => SubRequest::ReconnectAll,
          };

          if let Err(e) = channel.send((req, None)) {
            tracing::error!(?e)
          }
        }
      } else {
        loop {
          ping_timer.tick().await;
          if let Err(e) = channel.send((SubRequest::Ping, None)) {
            tracing::error!(?e)
          }
        }
      }
    };

    self.monitor_worker = Some(tokio::spawn(
      fut.instrument(tracing::debug_span!("monitor_worker_loop")),
    ));

    Ok(())
  }
}
