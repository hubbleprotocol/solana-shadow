use crate::{Pubkey, Result};
use solana_sdk::account::Account;
use std::time::Instant;
use tokio::sync::{
  watch::{self, Receiver, Sender},
  RwLock,
};

#[derive(Debug, Clone)]
pub struct AccountSnapshot {
  pub at: Instant,
  pub account: Account,
}

pub struct AccountShadow {
  address: Pubkey,
  latest: RwLock<AccountSnapshot>,
  channel: (Sender<AccountSnapshot>, Receiver<AccountSnapshot>),
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
      channel: watch::channel(init),
    }
  }

  pub const fn pubkey(&self) -> &Pubkey {
    &self.address
  }

  pub async fn update(&self, new_value: Account) -> Result<()> {
    let new_snapshot = AccountSnapshot {
      at: Instant::now(),
      account: new_value,
    };

    {
      let mut writer = self.latest.write().await;
      *writer = new_snapshot.clone();
    }

    if !self.channel.0.is_closed() {
      self.channel.0.send(new_snapshot)?;
    }

    Ok(())
  }

  pub async fn access<F>(&self, op: F)
  where
    F: Fn(&AccountSnapshot),
  {
    let read = self.latest.read().await;
    op(&*read);
  }

  pub fn subscribe(&self) -> Receiver<AccountSnapshot> {
    self.channel.1.clone()
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::AccountShadow;
  use solana_sdk::{account::Account, pubkey::Pubkey};

  #[tokio::test]
  async fn account_shadow_subscribe_and_update() {
    let account1 = Account::new(0, 0, &Pubkey::default());
    let account2 = Account::new(1, 1, &Pubkey::default());
    let shadow = Arc::new(AccountShadow::new(Pubkey::default(), account1));

    shadow
      .access(|acc| {
        assert_eq!(0, acc.account.lamports);
        assert_eq!(0, acc.account.data.len());
      })
      .await;

    let mut subscriber = shadow.subscribe();

    assert_eq!(0, subscriber.borrow().account.lamports);
    assert_eq!(0, subscriber.borrow().account.data.len());

    let s1 = shadow.clone();
    let handle1 = tokio::spawn(async move {
      s1.update(account2).await.unwrap();
    });

    let s2 = shadow.clone();
    let handle2 = tokio::spawn(async move {
      assert!(subscriber.changed().await.is_ok());
      s2.access(|acc| {
        assert_eq!(1, acc.account.lamports);
        assert_eq!(1, acc.account.data.len());
      })
      .await;
    });

    let (a, b) = tokio::join!(handle1, handle2);

    assert!(a.is_ok());
    assert!(b.is_ok());
  }
}
