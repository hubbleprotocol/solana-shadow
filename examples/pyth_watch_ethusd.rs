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

  let shadow = BlockchainShadow::new_from_accounts(
    &vec![
      "JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB".parse()?, // eth/usd
      "GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU".parse()?, // btc/usd
    ],
    Network::Mainnet,
  )
  .await?;

  shadow.for_each_account(|pubkey, acc| {
    println!("[{}]: account: {:?}", pubkey, acc);
  });

  shadow.wait().await?;

  Ok(())
}
