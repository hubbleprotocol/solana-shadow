use std::str::FromStr;

use anyhow::Result;
//use solana_shadow::{Network, ProgramMonitor, Pubkey};

#[tokio::main]
async fn main() -> Result<()> {
  // this account is the programid that owns all accounts
  // at https://pyth.network/developers/accounts/

  // let acc = Pubkey::from_str("gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s")?;
  // let pyth = ProgramMonitor::new(acc, Network::Devnet).await?;
  
  println!("I will monitor pyth eth/usd prices");

  Ok(())
}
