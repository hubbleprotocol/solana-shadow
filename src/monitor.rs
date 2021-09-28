use dashmap::DashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, trace};

use crate::{AccountShadow, Network, Result};

pub struct ClusterMonitor {
  network: Network,
  accounts: DashMap<Pubkey, AccountShadow>,
  subscriptions: DashMap<u64, Pubkey>,
}

// public methods
impl ClusterMonitor {
  pub async fn new_from_accounts(
    accounts: &[Pubkey],
    network: Network,
  ) -> Result<Self> {
    ClusterMonitor::new_from_account_shadows(
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
    let accounts = ClusterMonitor::accounts_graph(&program, &network).await?;
    trace!(
      "Initialized accounts graph: {:?}",
      &accounts
        .iter()
        .map(|acc| acc.pubkey())
        .collect::<Vec<&Pubkey>>()
    );
    ClusterMonitor::new_from_account_shadows(accounts, network).await
  }

  pub fn len(&self) -> usize {
    self.subscriptions.len()
  }
}

// internal methods
impl ClusterMonitor {
  async fn new_from_account_shadows(
    accounts: Vec<AccountShadow>,
    network: Network,
  ) -> Result<Self> {
    Ok(Self {
      network,
      accounts: accounts
        .into_iter()
        .map(|acc| (acc.pubkey().clone(), acc))
        .collect(),
      subscriptions: DashMap::new(),
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
