use std::path::PathBuf;
use transaction_processor::process_transactions;

fn test_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_cases")
}

#[test]
fn process_transactions_no_transactions_test() {
    let result = process_transactions(test_directory().join("no_transactions.csv")).unwrap();

    let expected = "client,available,held,total,locked";
    assert_eq!(result, expected)
}
#[test]
fn process_transactions_single_client_deposits_test() {
    let result = process_transactions(test_directory().join("single_client_deposits.csv")).unwrap();

    let expected = "client,available,held,total,locked\n\
    1,130.0,0.0,130.0,false\n";
    assert_eq!(result, expected)
}