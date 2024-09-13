use std::path::PathBuf;
use std::process::exit;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// Simple processor of transactions
/// Processes transactions in the input file and returns the account status after processing
struct Args {
    input_filepath: PathBuf
}

fn main() {
    let args = Args::parse();
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
