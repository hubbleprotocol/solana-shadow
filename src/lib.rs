mod blockchain;
mod error;
mod network;
mod program;
mod sync;
mod message;

pub use blockchain::BlockchainShadow;
pub use error::{Error, Result};
pub use network::Network;
pub use solana_sdk::pubkey::Pubkey;

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
