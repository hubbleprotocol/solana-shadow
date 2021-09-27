mod error;
mod network;
mod program;
mod account;

pub use error::{Error, Result};
pub use network::Network;
pub use account::AccountShadow;
pub use solana_sdk::pubkey::Pubkey;

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
