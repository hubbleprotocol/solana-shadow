mod sync;
mod account;
mod error;
mod network;
mod program;
mod blockchain;

pub use account::AccountShadow;
pub use error::{Error, Result};
pub use network::Network;
pub use solana_sdk::pubkey::Pubkey;
pub use blockchain::BlockchainShadow;

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
