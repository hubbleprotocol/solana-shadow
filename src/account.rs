use crate::Pubkey;
use solana_sdk::account::Account;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct AccountSnapshot {
  pub at: Instant,
  pub account: Account,
}

#[derive(Debug)]
pub struct AccountShadow {
  address: Pubkey,
  latest: RwLock<AccountSnapshot>,
}

impl AccountShadow {
  pub fn new(address: Pubkey, initial: Account) -> Self {
    let init = AccountSnapshot {
      at: Instant::now(),
      account: initial,
    };
    Self {
      address,
      latest: RwLock::new(init.clone()),
    }
  }

  pub const fn pubkey(&self) -> &Pubkey {
    &self.address
  }

  pub async fn update(&self, new_value: Account) {
    let new_snapshot = AccountSnapshot {
      at: Instant::now(),
      account: new_value,
    };

    let mut writer = self.latest.write().await;
    *writer = new_snapshot.clone();
  }

  pub async fn access<F>(&self, op: F)
  where
    F: Fn(&AccountSnapshot),
  {
    let read = self.latest.read().await;
    op(&*read);
  }
}

#[cfg(test)]
mod tests {
  use super::AccountShadow;
  use solana_sdk::{account::Account, pubkey::Pubkey};

  #[tokio::test]
  async fn account_shadow_access_and_update() {
    let account1 = Account::new(0, 0, &Pubkey::default());
    let account2 = Account::new(1, 1, &Pubkey::default());
    let shadow = AccountShadow::new(Pubkey::default(), account1);

    shadow
      .access(|acc| {
        assert_eq!(0, acc.account.lamports);
        assert_eq!(0, acc.account.data.len());
      })
      .await;

    shadow.update(account2).await;

    shadow
      .access(|acc| {
        assert_eq!(1, acc.account.lamports);
        assert_eq!(1, acc.account.data.len());
      })
      .await;
  }
}
