mod account;
mod error;
mod network;
mod program;
mod monitor;

pub use account::AccountShadow;
pub use error::{Error, Result};
pub use network::Network;
pub use solana_sdk::pubkey::Pubkey;
pub use monitor::ClusterMonitor;

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
