use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Network {
  Devnet,
  Testnet,
  Mainnet,
  Localhost,
  Custom(String),
}

impl Network {
  pub(crate) fn rpc_url(&self) -> String {
    match self {
      Network::Devnet => "https://api.devnet.solana.com",
      Network::Testnet => "https://api.testnet.solana.com",
      Network::Mainnet => "https://api.mainnet-beta.solana.com",
      Network::Localhost => "http://127.0.0.1:8899",
      Network::Custom(url) => url,
    }
    .to_owned()
  }
  pub(crate) fn wss_url(&self) -> String {
    match self {
      Network::Devnet => "https://api.devnet.solana.com",
      Network::Testnet => "https://api.testnet.solana.com",
      Network::Mainnet => "https://api.mainnet-beta.solana.com",
      Network::Localhost => "http://127.0.0.1:8900",
      Network::Custom(url) => url,
    }
    .to_owned()
  }
}

impl Display for Network {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.rpc_url())
  }
}
