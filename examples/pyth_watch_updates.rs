use std::time::Duration;

use anyhow::Result;
use pyth_client::{cast, Price};
use solana_shadow::{BlockchainShadow, Network};
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
  let solusd = "H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG".parse()?;

  // create an offline shadow of the on-chain data.
  // whenever the data change on-chain those changes
  // will be reflected immediately in this type.
  let shadow = BlockchainShadow::new_for_accounts(
    &vec![ethusd, btcusd, solusd],
    Network::Mainnet,
  )
  .await?;

  println!(
    "this example will start printing prices of {}",
    "ETH and BTC every time they change after 5 seconds"
  );
  println!();

  // get a mpmc receiver end of an updates channel
  let updates_channel = shadow.updates_channel()?;

  tokio::spawn(async move {

    // start printing updates only starting from the 5th second
    tokio::time::sleep(Duration::from_secs(5)).await;

    // now everytime an account changes, its pubkey will be
    // broadcasted to all receivers that are waiting on updates.
    while let Ok((pubkey, account)) = updates_channel.recv().await {
      let price = cast::<Price>(&account.data).agg.price;
      println!("account updated: {}: {}", &pubkey, price);
    }
  });

  shadow.worker().await?;
  Ok(())
}
