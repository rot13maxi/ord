use std::collections::HashMap;

use bitcoincore_rpc::bitcoincore_rpc_json::CreateRawTransactionInput;
use bitcoincore_rpc::json::AddressType;
use log::info;

use crate::subcommand::wallet::inscribe::Inscribe;

use super::*;

#[derive(Debug, Deserialize, Serialize)]
struct Brc20 {
    p: String,
    lim: String,
    op: String,
    tick: String,
    max: String,
}

#[derive(Serialize, Deserialize)]
pub struct Output {}

pub(crate) fn run(options: Options) -> SubcommandResult {
    let client = options.bitcoin_rpc_client_for_wallet_command(false)?;
    let mut transactions_checked = HashSet::new();
    loop {
        // TODO: pull threshold and fee rate into cli args instead of options?
        // fee rate is tricky because this is long running. maybe we should get it from the node?
        split_large_utxos(
            &client,
            Amount::from_sat(options.sophon_threshold.unwrap_or(50000)),
            FeeRate::try_from(options.sophon_fee_rate.unwrap_or(100.0))?,
        )?;

        for txid in client.get_raw_mempool().unwrap() {
            if transactions_checked.contains(&txid) {
                continue;
            }
            if let Ok(transaction) = client.get_raw_transaction(&txid, None) {
                transactions_checked.insert(txid);
                if let Ok(Some(brc20)) = process_transaction(&transaction) {
                    let original_fee_rate = calculate_fee_rate(&transaction, &client);
                    let target_fee_rate = original_fee_rate * 1.5;
                    // write the new brc20 to a file
                    let file = std::fs::File::create(format!("{}.txt", brc20.tick)).unwrap();
                    serde_json::to_writer(file, &brc20).unwrap();
                    // broadcast the new brc20
                    let inscription = Inscribe {
                        satpoint: None,
                        fee_rate: FeeRate::try_from(target_fee_rate)?,
                        commit_fee_rate: None,
                        file: format!("{}.txt", brc20.tick).parse()?,
                        no_backup: false,
                        no_limit: false,
                        dry_run: false,
                        destination: None,
                        postage: None,
                        parent: None,
                        reinscribe: false,
                        metaprotocol: None,
                    };
                    info!(
                        "Broadcasting new brc20: {:?} at fee rate {}",
                        brc20, target_fee_rate
                    );
                    inscription.run(options.clone())?;
                }
            }
        }
        // sleep for 10 seconds
        info!("Sleeping for 10 seconds...");
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}

// look in the wallet for unspent outputs larger than threshold and split them into outputs that are smaller than threshold
fn split_large_utxos(
    client: &Client,
    threshold: Amount,
    splitting_fee_rate: FeeRate,
) -> Result<(), Error> {
    let unspent_outputs = client.list_unspent(None, None, None, None, None)?;
    for unspent_output in unspent_outputs {
        if unspent_output.amount > threshold {
            info!("Found a UTXO worth {}. Attempting to split it into smaller outputs.", unspent_output.amount);
            let mut outputs = HashMap::new();
            let mut amount = unspent_output.amount;
            while amount > threshold {
                amount -= threshold;
                let address = client
                    .get_new_address(None, Some(AddressType::Bech32m))?
                    .assume_checked();
                outputs.insert(address.to_string(), threshold);
            }
            // create a using all the outputs to get an upper bound on fees. then we'll remove
            // outputs until we get to the fee we want to pay
            let fee_estimate_tx = client.create_raw_transaction(
                &[CreateRawTransactionInput {
                    txid: unspent_output.txid,
                    vout: unspent_output.vout,
                    sequence: None,
                }],
                &outputs,
                None,
                None,
            )?;
            let tx_vsize = fee_estimate_tx.vsize();
            let fee = splitting_fee_rate.fee(tx_vsize);
            let mut fee_paid = Amount::from_sat(0);
            let mut keys_to_remove = Vec::new();
            let mut iter = outputs.iter();

            while fee_paid < fee {
                if let Some((address, amount)) = iter.next() {
                    fee_paid = fee_paid.checked_add(*amount).unwrap();
                    keys_to_remove.push(address.clone());
                } else {
                    break;
                }
            }
            let remainder = fee_paid - fee;

            for address in keys_to_remove {
                outputs.remove(&address);
            }

            if remainder > Amount::from_sat(1000) {
                // if we have more than dust left, add it as an output
                outputs.insert(client
                                   .get_new_address(None, Some(AddressType::Bech32m))?
                                   .assume_checked().to_string(), remainder);
            }
            let transaction = client.create_raw_transaction(
                &[CreateRawTransactionInput {
                    txid: unspent_output.txid,
                    vout: unspent_output.vout,
                    sequence: None,
                }],
                &outputs,
                None,
                None,
            )?;
            let signed_tx = client.sign_raw_transaction_with_wallet(&transaction, None, None)?;

            match client.send_raw_transaction(&signed_tx.hex) {
                Ok(txid) => {
                    info!("Successfully broadcast UTXO splitting transaction: {}. Split it into {} outputs and paid a fee of {}", txid, outputs.len(), fee_paid);
                }
                Err(e) => {
                    info!("Error broadcasting UTXO splitting transaction: {}", e);
                }
            }
        }
    }
    Ok(())
}

fn calculate_fee_rate(transaction: &Transaction, client: &Client) -> f64 {
    let mut total_input_amount = 0;
    for input in transaction.input.clone() {
        let txid = input.previous_output.txid;
        let vout = input.previous_output.vout;
        let tx = client.get_raw_transaction(&txid, None).unwrap();
        let vout = tx.output.get(vout as usize).unwrap();
        total_input_amount += vout.value;
    }
    let total_output_amount = transaction
        .output
        .iter()
        .fold(0, |acc, output| acc + output.value);
    let fee = total_input_amount - total_output_amount;
    let vsize = transaction.vsize();
    let fee_rate = fee as f64 / vsize as f64;
    fee_rate
}

fn process_transaction(transaction: &Transaction) -> Result<Option<Brc20>> {
    info!("Checking transaction: {}", transaction.txid());

    let envelope = ParsedEnvelope::from_transaction(&transaction);

    if envelope.is_empty() {
        return Ok(None);
    }

    info!("Found inscription transaction: {}", transaction.txid());
    for inscription in envelope {
        if let Some(content_type) = inscription.payload.content_type {
            if String::from_utf8(content_type.to_ascii_lowercase())?
                == "text/plain;charset=utf-8".to_string()
            {
                let payload_body = inscription.payload.body.ok_or(anyhow!("no body"))?;
                info!(
                    "Found text/plain inscription: {:?}",
                    String::from_utf8(payload_body.clone())?
                );
                if let Ok(brc20) =
                    serde_json::from_str::<Brc20>(&String::from_utf8(payload_body.to_ascii_lowercase())?)
                {
                    info!("Found BRC20 inscription: {:?}", brc20);
                    if brc20.op == "deploy" && brc20.max != "1" {
                        let new_brc = Brc20 {
                            lim: "1".to_string(),
                            max: "1".to_string(),
                            ..brc20
                        };
                        info!(
                            "BRC-20 deployment detected. Deploying brc-20 with max=1: {:?}",
                            new_brc
                        );
                        return Ok(Some(new_brc));
                    }
                }
            }
        }
    }
    Ok(None)
}
