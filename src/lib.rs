use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::Context;
use csv::{ReaderBuilder, Trim, WriterBuilder};

use crate::models::{ClientSummary, Transaction};
use crate::processor::TransactionsProcessor;

mod models;
mod processor;

pub fn process_transactions(filename: impl AsRef<Path>) -> anyhow::Result<String> {
    let f = File::open(filename).context("Failed to open input file")?;
    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .has_headers(true)
        .from_reader(BufReader::new(f));

    let mut processor = TransactionsProcessor::default();
    for record in reader.deserialize() {
        let transaction: Transaction = record.context("Failed to deserialize transaction")?;
        // The errors from transactions are ignored in this function as if transaction has never happened
        let _ = processor.process(&transaction);
    }

    into_csv(processor.summary())
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
                .context("Failed to write summary record")?;
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
