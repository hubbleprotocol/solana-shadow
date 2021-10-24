use crate::{
  sync::{AccountUpdate, SolanaChangeListener, SubRequest},
  Error, Network, Result,
};
use dashmap::DashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::{sync::Arc, time::Duration};
use tokio::{
  sync::{
    broadcast::{self, Receiver, Sender},
    mpsc::{unbounded_channel, UnboundedSender},
  },
  task::JoinHandle,
};
use tracing::{debug, error, info};

type AccountsMap = DashMap<Pubkey, Account>;

/// This parameter control how many updates are going to be stored in memory
/// for update receivers (if any subscribed) before it starts returning dropping
/// them and returning RecvError::Lagging.
/// For more info see: https://docs.rs/tokio/1.12.0/tokio/sync/broadcast/index.html#lagging
/// for now it is set to 64, which gives us about 30 seconds on Solana
/// as there can be an update at most once every 400 miliseconds (blocktime)
const MAX_UPDATES_SUBSCRIBER_LAG: usize = 64;

pub struct SyncOptions {
  pub network: Network,
  pub max_lag: Option<usize>,
  pub reconnect_every: Option<Duration>,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      network: Network::Mainnet,
      max_lag: None,
      reconnect_every: None,
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
  sub_req: Option<UnboundedSender<SubRequest>>,
  sync_worker: Option<JoinHandle<Result<()>>>,
  monitor_worker: Option<JoinHandle<Result<()>>>,
  ext_updates: Sender<(Pubkey, Account)>,
}

// public methods
impl BlockchainShadow {
  pub async fn new(options: SyncOptions) -> Result<Self> {
    let max_lag = options.max_lag.unwrap_or(MAX_UPDATES_SUBSCRIBER_LAG);
    let mut instance = Self {
      options,
      accounts: Arc::new(AccountsMap::new()),
      sync_worker: None,
      monitor_worker: None,
      sub_req: None,
      ext_updates: broadcast::channel(max_lag).0,
    };

    instance.create_worker().await?;
    instance.create_monitor_worker().await?;

    Ok(instance)
  }

  pub async fn add_accounts(&mut self, accounts: &[Pubkey]) -> Result<()> {
    let initial: Vec<_> = RpcClient::new(self.network().rpc_url())
      .get_multiple_accounts(accounts)?
      .into_iter()
      .zip(accounts.iter())
      .filter(|(o, _)| o.is_some())
      .map(|(acc, key)| (*key, acc.unwrap()))
      .collect();

    for (key, acc) in initial {
      self.accounts.insert(key, acc);
      self
        .sub_req
        .clone()
        .unwrap()
        .send(SubRequest::Account(key))
        .map_err(|_| Error::InternalError)?;
    }

    Ok(())
  }

  pub async fn add_account(&mut self, account: &Pubkey) -> Result<()> {
    self.add_accounts(&[*account]).await
  }

  pub async fn add_program(&mut self, program_id: &Pubkey) -> Result<()> {
    let initial: Vec<_> = RpcClient::new(self.network().rpc_url())
      .get_program_accounts(&program_id)?
      .into_iter()
      .collect();

    for (key, acc) in initial {
      self.accounts.insert(key, acc);
    }
    self
      .sub_req
      .clone()
      .unwrap()
      .send(SubRequest::Program(*program_id))
      .map_err(|_| Error::InternalError)?;
    Ok(())
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

  pub fn for_each_account(&self, op: impl Fn(&Pubkey, &Account)) {
    for pair in self.accounts.iter() {
      let pubkey = pair.pair().0;
      let account = pair.pair().1;
      op(pubkey, &account);
    }
  }

  pub fn get_account(&self, key: &Pubkey) -> Option<Account> {
    match self.accounts.get(key) {
      // this is rw-locked
      None => None,
      Some(acc) => Some(acc.clone()),
    }
  }

  pub async fn worker(mut self) -> Result<()> {
    match self.sync_worker.take() {
      Some(handle) => Ok(handle.await??),
      None => Err(Error::WorkerDead),
    }
  }

  pub fn updates_channel(&self) -> Receiver<(Pubkey, Account)> {
    debug!(
      "-> sub -> receiver count: {}",
      self.ext_updates.receiver_count()
    );
    let sub = self.ext_updates.subscribe();
    debug!(
      "-> sub -> receiver count: {}",
      self.ext_updates.receiver_count()
    );
    sub
  }
}

impl BlockchainShadow {
  async fn create_worker(&mut self) -> Result<()> {
    // subscription requests from blockchain shadow -> listener
    let (subscribe_tx, mut subscribe_rx) = unbounded_channel::<SubRequest>();

    self.sub_req = Some(subscribe_tx);
    let network = self.network().clone();
    let accs_ref = self.accounts.clone();
    let updates_tx = self.ext_updates.clone();
    self.sync_worker = Some(tokio::spawn(async move {
      let mut listener = SolanaChangeListener::new(network).await?;
      loop {
        tokio::select! {
          recv_result = listener.recv() => {
            match recv_result {
              Ok(Some(AccountUpdate { pubkey, account })) => {
                debug!("account {} updated", &pubkey);
                accs_ref.insert(pubkey, account.clone());
                if updates_tx.receiver_count() != 0 {
                  updates_tx.send((pubkey, account)).unwrap();
                }
              },
              Ok(None) => {
                error!("Websocket connection to solana dropped");
              },
              Err(e) => error!("error in the sync worker thread: {:?}", e)
            }
          },
          Some(subreq) = subscribe_rx.recv() => {
            match subreq {
              SubRequest::Account(pubkey) => listener.subscribe_account(pubkey).await?,
              SubRequest::Program(pubkey) => listener.subscribe_program(pubkey).await?,
              SubRequest::ReconnectAll => listener.reconnect_all().await?
            }
          }
        };
      }
    }));

    Ok(())
  }

  async fn create_monitor_worker(&mut self) -> Result<()> {
    let channel = self.sub_req.clone();
    if let Some(every) = self.options.reconnect_every {
      self.monitor_worker = Some(tokio::spawn(async move {
        loop {
          let channel_clone = channel.clone();
          info!("reestablising connection to solana");
          tokio::time::sleep(every).await;
          channel_clone
            .unwrap()
            .send(SubRequest::ReconnectAll)
            .map_err(|_| Error::InternalError)?;
        }
      }));
    }
    Ok(())
  }
}
