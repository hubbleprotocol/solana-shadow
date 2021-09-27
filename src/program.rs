// use crate::{AccountShadow, Error, Network, Pubkey, Result};
// use dashmap::DashMap;
// use solana_client::rpc_client::RpcClient;

// pub struct ProgramMonitor {
//   program_id: Pubkey,
//   accounts: DashMap<Pubkey, AccountShadow>,
// }

// impl ProgramMonitor {
//   pub async fn new(program_id: Pubkey, network: Network) -> Result<Self> {
//     let rpc = RpcClient::new(network.rpc_url());
//     let acc_info = rpc.get_account(&program_id)?;

//     // this is intedend for monitoring an entire graph of
//     // accounts that are owned by a program, if the intention
//     // is o monitor a single account, then use AccountMonitor
//     // instead. Non-executable accoutns don't own accounts.
//     if !acc_info.executable {
//       return Err(Error::NotAProgramAccount);
//     }

//     let accounts = rpc.get_program_accounts(&program_id)?;

//     Ok(Self {
//       program_id,
//       accounts: DashMap::<Pubkey, AccountShadow>::new(),
//     })
//   }
// }
