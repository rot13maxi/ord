use {super::*};

use crate::subcommand::wallet::inscribe::Inscribe;

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
                    println!("Broadcasting new brc20: {:?} at fee rate {}", brc20, target_fee_rate);
                    inscription.run(options.clone())?;
                }
            }
        }
        // sleep for 10 seconds
        println!("Sleeping for 10 seconds...");
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
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
    let total_output_amount = transaction.output.iter().fold(0, |acc, output| acc + output.value);
    let fee = total_input_amount - total_output_amount;
    let vsize = transaction.vsize();
    let fee_rate = fee as f64 / vsize as f64;
    fee_rate
}

fn process_transaction(transaction: &Transaction) -> Result<Option<Brc20>> {
    println!("Checking transaction: {}", transaction.txid());

    let envelope = ParsedEnvelope::from_transaction(&transaction);

    if envelope.is_empty() {
        return Ok(None);
    }

    println!("Found inscription transaction: {}", transaction.txid());
    for inscription in envelope {
        if let Some(content_type) = inscription.payload.content_type {
            if String::from_utf8(content_type.to_ascii_lowercase())? == "text/plain;charset=utf-8".to_string() {
                let payload_body = inscription.payload.body.ok_or(anyhow!("no body"))?;
                println!("Found text/plain inscription: {:?}", String::from_utf8(payload_body.clone())?);
                if let Ok(brc20) = serde_json::from_str::<Brc20>(&String::from_utf8(payload_body.to_ascii_lowercase())?) {
                    println!("Found BRC20 inscription: {:?}", brc20);
                    if brc20.op == "deploy" && brc20.max != "1" {
                        let new_brc = Brc20 {
                            lim: "1".to_string(),
                            max: "1".to_string(),
                            ..brc20
                        };
                        println!("BRC-20 deployment detected. Deploying brc-20 with max=1: {:?}", new_brc);
                        return Ok(Some(new_brc));
                    }
                }
            }
        }
    }
    Ok(None)
}
