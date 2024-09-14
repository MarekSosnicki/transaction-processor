use std::collections::HashMap;

use boolinator::Boolinator;
use itertools::Itertools;

use crate::models::{ClientId, ClientSummary, Transaction, TransactionId, TransactionType};

/// To ensure 4 digits precision, internally the calculations are using rounded integers
type AmountType = i64;
const PRECISION: f64 = 10000.0;
fn f64_to_amount_type(v: f64) -> AmountType {
    (v * PRECISION).round() as AmountType
}

fn amount_type_to_f64(v: AmountType) -> f64 {
    (v as f64) / PRECISION
}

/// Struct representing details of the transaction in client history
struct TransactionRecord {
    amount: AmountType,
    status: TransactionStatus,
}

#[derive(PartialEq)]
/// Describes status of the transaction in user history
enum TransactionStatus {
    /// Transaction was successful and is valid, the founds are in available
    Processed,
    /// Transaction is under dispute, the founds are in held
    UnderDispute,
    /// Transaction is charged back, the transaction is ignored in held/total but client account is frozen
    ChargeBack,
}

#[derive(Default)]
/// ClientData contains current user state
struct ClientData {
    /// All transactions already processed by user in their current state
    transactions_history: HashMap<TransactionId, TransactionRecord>,
}

impl ClientData {
    /// Returns the available founds
    fn available(&self) -> f64 {
        amount_type_to_f64(
            self.transactions_history
                .values()
                .filter(|t| t.status == TransactionStatus::Processed)
                .map(|record| record.amount)
                .sum(),
        )
    }

    /// Returns the held founds (under dispute)
    fn held(&self) -> f64 {
        amount_type_to_f64(
            self.transactions_history
                .values()
                .filter(|t| t.status == TransactionStatus::UnderDispute)
                .map(|record| record.amount)
                .sum(),
        )
    }

    /// Returns true if there is at least one transaction with `TransactionStatus::ChargeBack` status
    fn locked(&self) -> bool {
        self.transactions_history
            .values()
            .any(|t| t.status == TransactionStatus::ChargeBack)
    }
}

#[derive(Default)]
pub(crate) struct TransactionsProcessor {
    clients_data: HashMap<ClientId, ClientData>,
}

#[derive(Debug, PartialEq, thiserror::Error)]
/// Error type from processing the transactions
pub(crate) enum TransactionProcessError {
    #[error("Not enough founds")]
    NotEnoughFoundsAvailable,

    #[error("Missing required amount value")]
    MissingAmountValue,

    #[error("Non positive amount in transaction")]
    NonPositiveAmountInTransaction,

    #[error("Transaction not found")]
    TransactionNotFound,

    #[error("Transaction already under dispute")]
    TransactionAlreadyUnderDispute,

    #[error("Transaction to be disputed was withdrawal")]
    CannotDisputeWithdrawal,

    #[error("Transaction not under dispute")]
    TransactionNotUnderDispute,

    #[error("Account Locked")]
    AccountLocked,

    #[error("Transaction already processed")]
    TransactionAlreadyProcessed,
}

impl TransactionsProcessor {
    /// Processes the transaction
    pub(crate) fn process(
        &mut self,
        transaction: &Transaction,
    ) -> Result<(), TransactionProcessError> {
        let client_entry = self.clients_data.entry(transaction.client).or_default();
        // Return immediately if account is locked
        (!client_entry.locked()).ok_or(TransactionProcessError::AccountLocked)?;

        match transaction.transaction_type {
            TransactionType::Deposit => {
                let amount = transaction
                    .amount
                    .ok_or(TransactionProcessError::MissingAmountValue)?;

                (amount > 0.0).ok_or(TransactionProcessError::NonPositiveAmountInTransaction)?;

                (!client_entry
                    .transactions_history
                    .contains_key(&transaction.transaction_id))
                .ok_or(TransactionProcessError::TransactionAlreadyProcessed)?;

                client_entry.transactions_history.insert(
                    transaction.transaction_id,
                    TransactionRecord {
                        amount: f64_to_amount_type(amount),
                        status: TransactionStatus::Processed,
                    },
                );
            }
            TransactionType::Withdrawal => {
                let amount = transaction
                    .amount
                    .ok_or(TransactionProcessError::MissingAmountValue)?;
                (amount > 0.0).ok_or(TransactionProcessError::NonPositiveAmountInTransaction)?;
                (amount <= client_entry.available())
                    .ok_or(TransactionProcessError::NotEnoughFoundsAvailable)?;
                (!client_entry
                    .transactions_history
                    .contains_key(&transaction.transaction_id))
                .ok_or(TransactionProcessError::TransactionAlreadyProcessed)?;

                // Withdrawals are saved as Transaction records with negative values
                client_entry.transactions_history.insert(
                    transaction.transaction_id,
                    TransactionRecord {
                        amount: f64_to_amount_type(-amount),
                        status: TransactionStatus::Processed,
                    },
                );
            }
            TransactionType::Dispute => {
                let entry = client_entry
                    .transactions_history
                    .get_mut(&transaction.transaction_id)
                    .ok_or(TransactionProcessError::TransactionNotFound)?;
                (entry.status == TransactionStatus::Processed)
                    .ok_or(TransactionProcessError::TransactionAlreadyUnderDispute)?;
                (entry.amount > 0).ok_or(TransactionProcessError::CannotDisputeWithdrawal)?;
                entry.status = TransactionStatus::UnderDispute
            }
            TransactionType::Resolve => {
                let entry = client_entry
                    .transactions_history
                    .get_mut(&transaction.transaction_id)
                    .ok_or(TransactionProcessError::TransactionNotFound)?;
                (entry.status == TransactionStatus::UnderDispute)
                    .ok_or(TransactionProcessError::TransactionNotUnderDispute)?;
                entry.status = TransactionStatus::Processed
            }
            TransactionType::Chargeback => {
                let entry = client_entry
                    .transactions_history
                    .get_mut(&transaction.transaction_id)
                    .ok_or(TransactionProcessError::TransactionNotFound)?;
                (entry.status == TransactionStatus::UnderDispute)
                    .ok_or(TransactionProcessError::TransactionNotUnderDispute)?;
                entry.status = TransactionStatus::ChargeBack
            }
        }

        Ok(())
    }

    /// Returns summary of client accounts after processing transactions
    pub(crate) fn summary(&self) -> Vec<ClientSummary> {
        self.clients_data
            .iter()
            .map(|(client_id, data)| {
                let available = data.available();
                let held = data.held();
                ClientSummary {
                    client: *client_id,
                    available,
                    held,
                    total: held + available,
                    locked: data.locked(),
                }
            })
            // Sorting added for consistent outputs, not strictly needed but simplifies the tests
            .sorted_by_key(|summary| summary.client)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_transactions_should_return_empty_summary() {
        let processor = TransactionsProcessor::default();
        assert_eq!(processor.summary(), vec![])
    }

    #[test]
    fn deposits_should_increase_total_and_available_values() {
        let mut processor = TransactionsProcessor::default();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(10.0),
            })
            .unwrap();
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 10.0,
                held: 0.0,
                total: 10.0,
                locked: false,
            }]
        );

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 2,
                amount: Some(123.123),
            })
            .unwrap();

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 133.123,
                held: 0.0,
                total: 133.123,
                locked: false,
            }]
        );
    }

    #[test]
    fn deposit_non_positive_value_should_fail() {
        let mut processor = TransactionsProcessor::default();
        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(-10.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::NonPositiveAmountInTransaction);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn deposit_the_same_transaction_twice_should_fail() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(10.0),
            })
            .unwrap();
        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(10.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionAlreadyProcessed);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 10.0,
                held: 0.0,
                total: 10.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn deposit_without_amount_should_fail() {
        let mut processor = TransactionsProcessor::default();
        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::MissingAmountValue);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn transactions_should_work_independently_for_users() {
        let mut processor = TransactionsProcessor::default();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(23.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 6,
                transaction_id: 2,
                amount: Some(123.123),
            })
            .unwrap();

        assert_eq!(
            processor.summary(),
            vec![
                ClientSummary {
                    client: 1,
                    available: 23.0,
                    held: 0.0,
                    total: 23.0,
                    locked: false,
                },
                ClientSummary {
                    client: 6,
                    available: 123.123,
                    held: 0.0,
                    total: 123.123,
                    locked: false,
                }
            ]
        );
    }

    #[test]
    fn withdrawal_should_decrease_total_and_available_values() {
        let mut processor = TransactionsProcessor::default();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 2,
                amount: Some(25.0),
            })
            .unwrap();

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 75.0,
                held: 0.0,
                total: 75.0,
                locked: false,
            }]
        );

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 3,
                amount: Some(75.0),
            })
            .unwrap();

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn withdrawal_should_fail_and_not_decrease_total_and_available_values_if_it_would_fall_below_0()
    {
        let mut processor = TransactionsProcessor::default();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 2,
                amount: Some(25.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::NotEnoughFoundsAvailable);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 2,
                amount: Some(20.0),
            })
            .unwrap();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 3,
                amount: Some(20.0001),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::NotEnoughFoundsAvailable);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 20.0,
                held: 0.0,
                total: 20.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn withdrawal_non_positive_value_should_fail() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();
        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 1,
                amount: Some(-10.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::NonPositiveAmountInTransaction);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 100.0,
                held: 0.0,
                total: 100.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn withdrawal_without_amount_should_fail() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();
        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::MissingAmountValue);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 100.0,
                held: 0.0,
                total: 100.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn withdrawal_the_same_transaction_twice_should_fail() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(10.0),
            })
            .unwrap();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 2,
                amount: Some(5.0),
            })
            .unwrap();
        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 2,
                amount: Some(5.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionAlreadyProcessed);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 5.0,
                held: 0.0,
                total: 5.0,
                locked: false,
            }]
        );
    }
    #[test]
    fn dispute_should_fail_if_there_is_no_related_transaction() {
        let mut processor = TransactionsProcessor::default();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionNotFound);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn dispute_should_fail_if_related_transaction_is_withdrawal() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 2,
                amount: Some(20.0),
            })
            .unwrap();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::CannotDisputeWithdrawal);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 80.0,
                held: 0.0,
                total: 80.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn dispute_should_fail_if_related_transaction_is_already_under_dispute() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionAlreadyUnderDispute);
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 100.0,
                total: 100.0,
                locked: false,
            }]
        );
    }
    #[test]
    fn dispute_should_increase_the_held_amount_and_reduce_available() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 2,
                amount: Some(30.0),
            })
            .unwrap();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap();
        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 100.0,
                held: 30.0,
                total: 130.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn resolve_should_fail_if_there_is_no_related_transaction() {
        let mut processor = TransactionsProcessor::default();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Resolve,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionNotFound);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn resolve_should_fail_if_transaction_is_not_under_dispute() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Resolve,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionNotUnderDispute);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 100.0,
                held: 0.0,
                total: 100.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn resolve_should_revert_the_given_dispute() {
        // Creates two deposits, resolves only one
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 2,
                amount: Some(30.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Resolve,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap();

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 30.0,
                held: 100.0,
                total: 130.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn chargeback_should_revert_the_given_deposit_under_despute() {
        // Creates two deposits, disputes both, chargebacks the second one
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 2,
                amount: Some(30.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Chargeback,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap();

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 100.0,
                total: 100.0,
                locked: true,
            }]
        );
    }

    #[test]
    fn after_chargeback_no_transaction_should_be_processed() {
        // Creates a deposits, disputes and charges back then tries few transactions for the same client
        // and all should fail with the same error
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap();

        processor
            .process(&Transaction {
                transaction_type: TransactionType::Chargeback,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 5,
                amount: Some(100.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::AccountLocked);

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                transaction_id: 3,
                amount: Some(100.0),
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::AccountLocked);

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Dispute,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::AccountLocked);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: true,
            }]
        );
    }

    #[test]
    fn chargeback_should_fail_if_there_is_no_related_transaction() {
        let mut processor = TransactionsProcessor::default();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Chargeback,
                client: 1,
                transaction_id: 2,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionNotFound);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false,
            }]
        );
    }

    #[test]
    fn chargeback_should_fail_if_transaction_is_not_under_dispute() {
        let mut processor = TransactionsProcessor::default();
        processor
            .process(&Transaction {
                transaction_type: TransactionType::Deposit,
                client: 1,
                transaction_id: 1,
                amount: Some(100.0),
            })
            .unwrap();

        let err = processor
            .process(&Transaction {
                transaction_type: TransactionType::Chargeback,
                client: 1,
                transaction_id: 1,
                amount: None,
            })
            .unwrap_err();
        assert_eq!(err, TransactionProcessError::TransactionNotUnderDispute);

        assert_eq!(
            processor.summary(),
            vec![ClientSummary {
                client: 1,
                available: 100.0,
                held: 0.0,
                total: 100.0,
                locked: false,
            }]
        );
    }
}
