use anyhow::Result;
use solana_shadow::{BlockchainShadow, Network};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::Subscriber::builder()
    .with_writer(std::io::stdout)
    .with_env_filter(EnvFilter::try_from_default_env()?)
    .init();

  // this is the prog id that owns all pyth oracles on mainnet
  let prog = "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH".parse()?;
  let network = Network::Mainnet;
  let local = BlockchainShadow::new_for_program(&prog, network).await?;

  local.for_each_account(|pubkey, account| {
    println!(" - [{}]: {:?}", pubkey, account);
  });

  local.worker().await?;

  println!("I will monitor pyth eth/usd prices");

  Ok(())
}
