use anyhow::Result;
use solana_shadow::{BlockchainShadow, Network};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

fn configure_logging() {
  tracing_subscriber::fmt::Subscriber::builder()
    .with_writer(std::io::stdout)
    .with_env_filter(
      EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")),
    )
    .init();
}

#[tokio::main]
async fn main() -> Result<()> {
  configure_logging();

  println!("this example will dump the values of all accounts owned by pyth program every 5 seconds");
  println!();

  // this is the prog id that owns all pyth oracles on mainnet
  let prog = "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH".parse()?;
  let network = Network::Mainnet;
  let local = BlockchainShadow::new_for_program(&prog, network).await?;

  for _ in 0.. {
    local.for_each_account(|pubkey, account| {
      println!(" - [{}]: {:?}", pubkey, account);
    });

    tokio::time::sleep(Duration::from_secs(3)).await;
  }

  local.worker().await?;

  println!("I will monitor pyth eth/usd prices");

  Ok(())
}
