use std::str::FromStr;

use anyhow::Result;
use solana_shadow::{BlockchainShadow, Network};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::Subscriber::builder()
    .with_writer(std::io::stdout)
    .with_env_filter(EnvFilter::try_from_default_env()?)
    .init();

  let prog = "oraogph9PTJAYMfhpmkxNfG6TSCftK4kQyqaNU5YXao".parse()?;
  let network = Network::Devnet;
  let local = BlockchainShadow::new_from_program_id(&prog, network).await?;

  local.for_each_account(|pubkey, account| {
    println!("[{}]: {:?}", pubkey, account);
  });

  local.wait().await?;

  println!("I will monitor pyth eth/usd prices");

  Ok(())
}
