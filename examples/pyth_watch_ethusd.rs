use std::str::FromStr;

use anyhow::Result;
use solana_shadow::{ClusterMonitor, Network, Pubkey};

#[tokio::main]
async fn main() -> Result<()> {
  //
  // https://pyth.network/developers/accounts/
  //
  let _acc = Pubkey::from_str("2ciUuGZiee5macAMeQ7bHGTJtwcYTgnt6jdmQnnKZrfu")?;

  let monitor1 = ClusterMonitor::new_from_accounts(
    &vec![
      Pubkey::from_str("J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix")?, // eth/usd
      Pubkey::from_str("HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J")?, // btc/usd
    ],
    Network::Devnet,
  )
  .await?;
  
  let monitor2 = ClusterMonitor::new_from_program_id(
    &Pubkey::from_str("gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s")?,
    Network::Devnet,
  )
  .await?;

  Ok(())
}
