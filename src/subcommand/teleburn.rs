use super::*;
use crate::teleburn_address::EthereumTeleburnAddress;

#[derive(Debug, Parser)]
pub(crate) struct Teleburn {
  reverse: bool,
  recipient: InscriptionId,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Output {
  ethereum: EthereumTeleburnAddress,
}

impl Teleburn {
  pub(crate) fn run(self) -> Result {
    print_json(Output {
      ethereum: self.recipient.into(),
    })?;
    Ok(())
  }
}
