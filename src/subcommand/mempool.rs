use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Mempool {}

#[derive(Serialize, Deserialize)]
pub struct Output {
  pub inscription: InscriptionId,
  pub location: SatPoint,
  pub explorer: String,
}

impl Mempool {
  pub(crate) fn run(self, options: Options) -> Result {
    // let mut output = Vec::new();
    let inscriptions: Vec<Inscription> = options
      .bitcoin_rpc_client_for_wallet_command(false)?
      .get_raw_mempool()?
      .iter()
      .filter_map(|txid| {
        Inscription::from_transaction(
          &options
            .bitcoin_rpc_client_for_wallet_command(false)
            .unwrap()
            .get_transaction(txid, None)
            .unwrap()
            .transaction()
            .unwrap(),
        )
      })
      .collect();

    println!(
      "number of inscriptions in the mempool: {}",
      inscriptions.len()
    );

    // print_json(output)?;

    Ok(())
  }
}
