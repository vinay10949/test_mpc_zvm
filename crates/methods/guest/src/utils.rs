use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub amount: u64,
}
// Define a struct for Ethereum accounts
#[derive(Debug, Clone ,PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct EthAccount {
    pub address: String,
    pub balance: u64,
    pub storage_root: String,
}



// Function to generate the hash of the root node of the account storage trie
pub fn generate_account_storage_root(account_storage: &EthAccount) -> String {
    let account_storage_json = serde_json::to_string(account_storage).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(account_storage_json);
    let value =hasher.finalize();
    let hex_value: String = value.iter().map(|byte| format!("{:02x}", byte)).collect();
    hex_value
}



// Function to generate the account state root hash
pub fn generate_account_state_root_hash(accounts: &BTreeMap<String, EthAccount>) -> String {
    let account_states: Vec<_> = accounts
        .iter()
        .map(|(_, account)| {
            let account_json = serde_json::to_string(account).unwrap();
            let mut hasher = Sha256::new();
            hasher.update(account_json.as_bytes());
            let value =hasher.finalize();
            let hex_value: String = value.iter().map(|byte| format!("{:02x}", byte)).collect();
            hex_value
        })
        .collect();

    generate_merkle_root(&account_states)
}

pub fn generate_merkle_root(transaction_hashes: &[String]) -> String {
    let concatenated_hashes = transaction_hashes.concat();
    let mut hasher = Sha256::new();
    hasher.update(concatenated_hashes.as_bytes());
    let value =hasher.finalize();
    let hex_value: String = value.iter().map(|byte| format!("{:02x}", byte)).collect();
    hex_value
}
