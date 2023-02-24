use super::*;
use crate::subcommand::teleburn::Output::{Ethereum, Inscription};
use crate::teleburn_address::EthereumTeleburnAddress;

#[derive(Debug, Parser)]
pub(crate) struct Teleburn {
  #[clap(long, help = "Look up an inscription ID by teleburn address.")]
  reverse: Option<String>,
  #[clap(long, help = "Look up a teleburn address for an inscription.")]
  inscription_id: Option<InscriptionId>,
}

#[derive(Debug, PartialEq, Serialize)]
pub(crate) enum Output {
  Ethereum(EthereumTeleburnAddress),
  Inscription(InscriptionId),
}

impl Teleburn {
  pub(crate) fn run(self, options: Options) -> Result {
    if let Some(teleburn_addr) = self.reverse {
      let index = Index::open(&options)?;
      index.update()?;
      if let Some(inscription_id) = index.get_inscription_id_by_teleburn(&teleburn_addr)? {
        print_json(Inscription(inscription_id))?;
      }
    }
    if let Some(inscription_id) = self.inscription_id {
      print_json(Ethereum(inscription_id.into()))?;
    }
    Ok(())
  }
}
