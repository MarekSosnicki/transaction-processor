use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

pub(crate) type ClientId = u64;
pub(crate) type TransactionId = u64;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Transaction {
    #[serde(rename = "type")]
    pub(crate) transaction_type: TransactionType,
    pub(crate) client: ClientId,
    #[serde(rename = "tx")]
    pub(crate) transaction_id: TransactionId,
    pub(crate) amount: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct ClientSummary {
    pub(crate) client: ClientId,
    pub(crate) available: f64,
    pub(crate) held: f64,
    pub(crate) total: f64,
    pub(crate) locked: bool,
}
// TODO: Formatting for f64
