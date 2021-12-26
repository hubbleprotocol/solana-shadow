use std::{thread, time::Duration};
use tokio::sync::oneshot::channel;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
  account::Account,
  commitment_config::{CommitmentConfig, CommitmentLevel},
  pubkey::Pubkey,
};

use crate::error::Result;

/// Helper to convert sync code to async using an oneshot channel
/// uses a timeout to ensure continuation of executing in case
/// sync_code would panic
async fn build_async<F, T>(timeout: Duration, sync_code: F) -> Result<T>
where
  F: FnOnce() -> T + Send + 'static,
  T: Send + std::fmt::Debug + 'static,
{
  let (tx, rx) = channel::<T>();
  thread::spawn(move || {
    // TODO: if the sync_code panics this
    //       call will wait forever
    let r = sync_code();
    tx.send(r).expect("Unable to send oneshot")
  });

  Ok(tokio::time::timeout(timeout, rx).await??)
}

#[derive(Clone)]
pub struct ClientBuilder {
  rpc_url: String,
  rpc_timeout: Duration,
  commitment: CommitmentLevel,
}

impl ClientBuilder {
  pub fn new(
    rpc_url: String,
    rpc_timeout: Duration,
    commitment: CommitmentLevel,
  ) -> Self {
    Self {
      rpc_url,
      rpc_timeout,
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

#[allow(unused)]
pub async fn get_multiple_accounts(
  client: ClientBuilder,
  accounts: &[Pubkey],
) -> Result<Vec<(Pubkey, Account)>> {
  let unvalidated = {
    let accounts = accounts.to_vec();
    let rpc_client = client.build();
    build_async(client.rpc_timeout, move || {
      rpc_client.get_multiple_accounts(&accounts)
    })
    .await??
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

pub async fn get_account(
  client: ClientBuilder,
  account: Pubkey,
) -> Result<(Pubkey, Account)> {
  let rpc_client = client.build();
  Ok((
    account,
    build_async(client.rpc_timeout, move || rpc_client.get_account(&account))
      .await??,
  ))
}

pub async fn get_program_accounts(
  client: ClientBuilder,
  program_id: &Pubkey,
) -> Result<Vec<(Pubkey, Account)>> {
  let accounts = {
    let program_id = *program_id;
    let rpc_client = client.build();
    build_async(client.rpc_timeout, move || {
      rpc_client.get_program_accounts(&program_id)
    })
    .await??
  };

  Ok(accounts)
}
