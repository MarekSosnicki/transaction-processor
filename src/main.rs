use std::path::PathBuf;
use std::process::exit;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// Simple processor of transactions
/// Processes transactions in the input file and returns the account status after processing
struct Args {
    input_filepath: PathBuf,
}

const LOGS_FILENAME: &str = "transaction-processor-logs.log";

fn main() {
    let args = Args::parse();
    simple_logging::log_to_file(LOGS_FILENAME, log::LevelFilter::Info)
        .expect("Failed to start logging");
    match transaction_processor::process_transactions(args.input_filepath) {
        Ok(transactions_summary) => {
            println!("{}", transactions_summary);
        }
        Err(err) => {
            eprintln!("Failed to process input {:?}", err);
            exit(1)
        }
    }
}
