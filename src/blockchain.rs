use std::sync::Arc;

use crate::{
  sync::{SolanaChange, SolanaChangeListener},
  AccountShadow, Error, Network, Result,
};
use dashmap::DashMap;
use solana_client::{client_error::reqwest::Url, rpc_client::RpcClient};
use solana_sdk::pubkey::Pubkey;

use tokio::task::JoinHandle;
use tracing::{debug, trace};

pub struct BlockchainShadow {
  network: Network,
  sync_worker: Option<JoinHandle<()>>,
  accounts: Arc<DashMap<Pubkey, AccountShadow>>,
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
        .map(|(acc, key)| AccountShadow::new(*key, acc.unwrap()))
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
      &accounts
        .iter()
        .map(|acc| acc.pubkey())
        .collect::<Vec<&Pubkey>>()
    );
    BlockchainShadow::new_from_account_shadows(accounts, network).await
  }

  pub const fn network(&self) -> &Network {
    &self.network
  }

  pub fn len(&self) -> usize {
    self.accounts.len()
  }

  pub async fn wait(mut self) -> Result<()> {
    if let Some(handle) = self.sync_worker.take() {
      handle.await?;
    }

    Ok(())
  }
}

// internal methods
impl BlockchainShadow {
  async fn new_from_account_shadows(
    accounts: Vec<AccountShadow>,
    network: Network,
  ) -> Result<Self> {
    let pubkeys: Vec<Pubkey> =
      accounts.iter().map(|a| a.pubkey().clone()).collect();

    let accounts: Arc<DashMap<Pubkey, AccountShadow>> = Arc::new(
      accounts
        .into_iter()
        .map(|acc| (acc.pubkey().clone(), acc))
        .collect(),
    );

    let listener = SolanaChangeListener::new(&pubkeys, network.clone())?;
    let worker = tokio::spawn(async move {
      let mut listener = listener;

      while let Some(change) = listener.recv().await {
        trace!("recived blockchain update: {:?}", &change);

        match change {
          SolanaChange::Account(acc) => trace!("account changed: {:?}", &acc),
          SolanaChange::ProgramChange(prog) => {
            trace!("program changed: {:?}", &prog)
          }
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
  ) -> Result<Vec<AccountShadow>> {
    debug!("Initializing accounts graph for program {}", &program_id);
    Ok(
      RpcClient::new(network.rpc_url())
        .get_program_accounts(&program_id)?
        .into_iter()
        .map(|(key, acc)| AccountShadow::new(key, acc))
        .collect(),
    )
  }
}
