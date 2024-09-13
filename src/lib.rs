mod models;

use crate::models::{ClientId, ClientSummary, Transaction};
use anyhow::Context;
use csv::{ReaderBuilder, Trim, WriterBuilder};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn process_transactions(filename: impl AsRef<Path>) -> anyhow::Result<String> {
    let f = File::open(filename).context("Failed to open input file")?;
    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .has_headers(true)
        .from_reader(BufReader::new(f));

    let mut clients: HashMap<ClientId, ClientSummary> = Default::default();

    for record in reader.deserialize() {
        let transaction: Transaction = record.context("Failed to deserialize transaction")?;

        let entry = clients
            .entry(transaction.client)
            .or_insert_with(|| ClientSummary {
                client: transaction.client,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            });

        entry.available += transaction.amount;
        entry.total += transaction.amount;
    }

    into_csv(clients.values().cloned().collect())
}

fn into_csv(all_summaries: Vec<ClientSummary>) -> anyhow::Result<String> {
    if all_summaries.is_empty() {
        // serialize does not add headers if the records are empty
        Ok("client,available,held,total,locked".to_string())
    } else {
        let mut writer = WriterBuilder::new().from_writer(vec![]);

        for summary in all_summaries {
            writer
                .serialize(summary)
                .context("Failed to write summary record")?
        }
        let data = String::from_utf8(
            writer
                .into_inner()
                .context("Failed to get buffer from writer")?,
        )
        .context("Failed to convert buffer to string")?;
        Ok(data)
    }
}
