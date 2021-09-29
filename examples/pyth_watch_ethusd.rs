use anyhow::Result;
use pyth_client::{cast, Price};
use solana_shadow::{BlockchainShadow, Network};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt::Subscriber::builder()
    .with_writer(std::io::stdout)
    .with_env_filter(EnvFilter::try_from_default_env()?)
    .init();

  // https://pyth.network/developers/accounts/
  let ethusd = "JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB".parse()?;
  let btcusd = "GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU".parse()?;

  // create an offline shadow of the on-chain data.
  // whenever the data change on-chain those changes
  // will be reflected immediately in this type.
  let shadow = BlockchainShadow::new_from_accounts(
    &vec![ethusd, btcusd],
    Network::Mainnet,
  )
  .await?;

  println!("this example will print prices of ETH and BTC every 3 seconds");

  // iterate over the offline shadow of the account
  // everytime any account is accessed, then its contents
  // will reflect the latest version on-chain.
  for _ in 0.. {
    let ethacc = &shadow.get_account(&ethusd).unwrap();
    let ethprice = cast::<Price>(&ethacc.data).agg.price;

    let btcacc = &shadow.get_account(&btcusd).unwrap();
    let btcprice = cast::<Price>(&btcacc.data).agg.price;

    println!("ETH/USD: {}", ethprice);
    println!("BTC/USD: {}", btcprice);
    println!();
    
    tokio::time::sleep(Duration::from_secs(3)).await;
  }

  shadow.wait().await?;
  Ok(())
}
