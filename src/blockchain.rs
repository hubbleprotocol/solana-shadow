use std::sync::Arc;

use crate::{
  sync::{SolanaChange, SolanaChangeListener},
  Network, Result,
};
use dashmap::DashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};

use tokio::task::JoinHandle;
use tracing::{debug, trace, warn};

type AccountsMap = DashMap<Pubkey, Account>;

pub struct BlockchainShadow {
  network: Network,
  accounts: Arc<AccountsMap>,
  sync_worker: Option<JoinHandle<()>>,
}

// public methods
impl BlockchainShadow {
  pub async fn new_from_accounts(
    accounts: &[Pubkey],
    network: Network,
  ) -> Result<Self> {
    BlockchainShadow::new_from_account_shadows(
      RpcClient::new(network.rpc_url())
        .get_multiple_accounts(accounts)?
        .into_iter()
        .zip(accounts.iter())
        .filter(|(o, _)| o.is_some())
        .map(|(acc, key)| (*key, acc.unwrap()))
        .collect(),
      network,
    )
    .await
  }

  pub async fn new_from_program_id(
    program: &Pubkey,
    network: Network,
  ) -> Result<Self> {
    let accounts = BlockchainShadow::accounts_graph(&program, &network).await?;
    trace!(
      "Initialized accounts graph: {:?}",
      &accounts.iter().map(|(k, _)| k).collect::<Vec<&Pubkey>>()
    );
    BlockchainShadow::new_from_account_shadows(accounts, network).await
  }

  pub const fn network(&self) -> &Network {
    &self.network
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

  pub async fn wait(mut self) -> Result<()> {
    if let Some(handle) = self.sync_worker.take() {
      handle.await?;
    }

    Ok(())
  }
}

// internal associated methods
impl BlockchainShadow {
  async fn new_from_account_shadows(
    accounts: Vec<(Pubkey, Account)>,
    network: Network,
  ) -> Result<Self> {
    let listener = SolanaChangeListener::new(network.clone()).await?;
    let accounts: Arc<AccountsMap> = Arc::new(accounts.into_iter().collect());

    let accounts_ref = accounts.clone();
    let worker = tokio::spawn(async move {
      let accounts = accounts_ref;
      let mut listener = listener;

      // init subscriptions for all accounts
      for kv in accounts.iter() {
        match listener.subscribe(*kv.key()).await {
          Ok(_) => debug!("subscribing to account: {}", kv.key()),
          Err(e) => warn!("subscription to {} failed: {:?}", kv.key(), e),
        };
      }

      loop {
        match listener.recv().await {
          Ok(Some(change)) => Self::on_solana_change(accounts.clone(), change),
          Ok(None) => warn!("listener stream closed."), // todo: implement retry logic
          Err(e) => warn!("recv error: {:?}", e),
        }
      }
    });

    Ok(Self {
      network: network.clone(),
      sync_worker: Some(worker),
      accounts: accounts.clone(),
    })
  }

  async fn accounts_graph(
    program_id: &Pubkey,
    network: &Network,
  ) -> Result<Vec<(Pubkey, Account)>> {
    debug!("Initializing accounts graph for program {}", &program_id);
    Ok(
      RpcClient::new(network.rpc_url())
        .get_program_accounts(&program_id)?
        .into_iter()
        .collect(),
    )
  }

  fn on_solana_change(accounts: Arc<AccountsMap>, change: SolanaChange) {
    debug!("processing solana change: {:?}", &change);
    match change {
      SolanaChange::Account((key, acc)) => {
        debug!("account {} changed: {:?}", &key, &acc);
        accounts.insert(key, acc);
      }
      SolanaChange::ProgramChange(prog) => {
        debug!("program changed: {:?}", &prog)
      }
    };
  }
}
