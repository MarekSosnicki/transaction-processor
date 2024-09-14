use std::fs;
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

#[test]
fn process_multiple_users_all_types_of_transactions_test() {
    // This test has 4 client
    // Client1: Deposits some founds then tries to withdraw too much (account balance not negative)
    // Client2: Deposits some founds and has a few disputes
    // Client3: Deposits some founds, withdraws and then does chargeback ends with locked account and negative founds
    // Client4: Deposits some founds, does a chargeback and then account is locked a few transactions after should have no effect
    let result =
        process_transactions(test_directory().join("multiple_users_all_types_of_transactions.csv"))
            .unwrap();

    let expected = fs::read_to_string(
        test_directory().join("expected_multiple_users_all_types_of_transactions.csv"),
    )
    .unwrap()
    // Hack for windows
    .replace("\r\n", "\n");
    assert_eq!(result, expected)
}
