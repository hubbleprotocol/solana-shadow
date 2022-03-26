use std::{convert::TryFrom, io::Read, str::FromStr};

use serde::Deserialize;
use solana_sdk::{account::Account, pubkey::Pubkey};

#[derive(Debug, Deserialize)]
pub(crate) struct AccountChangeInfo {
  #[allow(dead_code)]
  pub value: NotificationValue,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProgramChangeInfo {}

#[derive(Debug, Deserialize)]
pub(crate) struct NotificationContext {
  #[allow(dead_code)]
  pub slot: u64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum NotificationValue {
  Account(AccountRepresentation),
  Program(WrappedAccountRepresentation),
}

#[derive(Debug, Deserialize)]
pub(crate) struct WrappedAccountRepresentation {
  pub pubkey: String,
  pub account: AccountRepresentation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountRepresentation {
  owner: String,
  executable: bool,
  lamports: u64,
  rent_epoch: u64,
  data: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NotificationResult {
  #[allow(dead_code)]
  pub context: NotificationContext,
  pub value: NotificationValue,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NotificationParams {
  pub result: NotificationResult,
  pub subscription: u64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SolanaMessage {
  Confirmation {
    #[allow(dead_code)]
    jsonrpc: String,
    result: u64,
    id: u64,
  },
  Notification {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: NotificationParams,
  },
}

impl TryFrom<AccountRepresentation> for Account {
  type Error = crate::Error;
  fn try_from(repr: AccountRepresentation) -> crate::Result<Self> {
    let data = match &repr.data[..] {
      [content, format] => match &format[..] {
        "base64" => base64::decode(&content)?,
        "base64+zstd" => {
          let zstd_data = base64::decode(content.as_bytes())?;

          let mut data = vec![];
          let mut reader =
            zstd::stream::read::Decoder::new(zstd_data.as_slice())?;
          reader.read_to_end(&mut data)?;
          data
        }
        _ => vec![],
      },
      _ => vec![],
    };

    Ok(Account {
      lamports: repr.lamports,
      data,
      owner: Pubkey::from_str(&repr.owner)?,
      executable: repr.executable,
      rent_epoch: repr.rent_epoch,
    })
  }
}
