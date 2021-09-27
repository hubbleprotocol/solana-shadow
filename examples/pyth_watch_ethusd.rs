use std::str::FromStr;

use anyhow::Result;
use solana_shadow::{AccountShadow, Network, Pubkey};

#[tokio::main]
async fn main() -> Result<()> {
  //
  // https://pyth.network/developers/accounts/
  //
  let _acc = Pubkey::from_str("2ciUuGZiee5macAMeQ7bHGTJtwcYTgnt6jdmQnnKZrfu")?;
  //let ethusd = AccountShadow::new(acc, None);

  Ok(())
}
