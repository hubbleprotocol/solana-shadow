use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Network {
  Devnet,
  Testnet,
  Mainnet,
  Localhost,
  /// (rpc_url, wss_url)
  Custom(String, String),
}

impl Network {
  pub(crate) fn rpc_url(&self) -> String {
    match self {
      Network::Devnet => "https://api.devnet.solana.com",
      Network::Testnet => "https://api.testnet.solana.com",
      Network::Mainnet => "https://api.mainnet-beta.solana.com",
      Network::Localhost => "http://127.0.0.1:8899",
      Network::Custom(url, _) => url,
    }
    .to_owned()
  }
  pub(crate) fn wss_url(&self) -> String {
    match self {
      Network::Devnet | Network::Testnet | Network::Mainnet => self.rpc_url(),
      Network::Localhost => "http://127.0.0.1:8900".to_owned(),
      Network::Custom(_, url) => url.to_owned(),
    }
  }
}

impl Display for Network {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.rpc_url())
  }
}
