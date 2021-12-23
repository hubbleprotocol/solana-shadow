use anyhow::Result;
use std::thread;
use tokio::sync::oneshot::channel;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
  account::Account,
  commitment_config::{CommitmentConfig, CommitmentLevel},
  pubkey::Pubkey,
};

/// Helper to convert sync code to async using an oneshot channel
/// TODO: improve this with a timeout
async fn build_async<F, T>(sync_code: F) -> Result<T>
where
  F: FnOnce() -> T + Send + 'static,
  T: Send + 'static,
{
  let (tx, rx) = channel::<T>();
  thread::spawn(move || {
    let r = sync_code();
    if let Err(_) = tx.send(r) {
      tracing::error!("oneshot error");
    };
  });

  rx.await.map_err(|e| anyhow::anyhow!("RecvError: {:?}", e))
}

#[derive(Clone)]
pub struct ClientBuilder {
  rpc_url: String,
  commitment: CommitmentLevel,
}

impl ClientBuilder {
  pub fn new(rpc_url: String, commitment: CommitmentLevel) -> Self {
    Self {
      rpc_url,
      commitment,
    }
  }

  fn build(&self) -> RpcClient {
    RpcClient::new_with_commitment(
      self.rpc_url.clone(),
      CommitmentConfig {
        commitment: self.commitment,
      },
    )
  }
}

pub async fn get_multiple_accounts(
  client: ClientBuilder,
  accounts: &[Pubkey],
) -> Result<Vec<(Pubkey, Account)>> {
  let unvalidated = {
    let accounts = accounts.to_vec();
    let client = client.build();
    build_async(move || client.get_multiple_accounts(&accounts)).await??
  };

  let valid_accounts: Vec<_> = unvalidated
    .into_iter()
    .zip(accounts.iter())
    .filter(|(o, _)| o.is_some())
    .map(|(acc, key)| (*key, acc.unwrap()))
    .collect();

  if valid_accounts.len() < accounts.len() {
    tracing::warn!(
      missing = %(valid_accounts.len() - accounts.len()),
      "non-existing accounts detected"
    )
  }

  Ok(valid_accounts)
}
