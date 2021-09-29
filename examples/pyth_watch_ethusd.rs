use anyhow::Result;
use solana_shadow::{BlockchainShadow, Network};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  //
  // https://pyth.network/developers/accounts/
  //
  tracing_subscriber::fmt::Subscriber::builder()
    .with_writer(std::io::stdout)
    .with_env_filter(EnvFilter::try_from_default_env()?)
    .init();

  let shadow1 = BlockchainShadow::new_from_accounts(
    &vec![
      "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix".parse()?, // eth/usd
      "HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J".parse()?, // btc/usd
    ],
    Network::Devnet,
  )
  .await?;

  shadow1.for_each_account(|pubkey, acc| {
    println!("[{}]: account: {:?}", pubkey, acc);
  });

  shadow1.wait().await?;

  Ok(())
}
