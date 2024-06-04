use std::collections::HashMap;

use hex::encode;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub amount: u64,
}
// Define a struct for Ethereum accounts
#[derive(Debug, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct EthAccount {
    pub address: String,
    pub balance: u64,
    pub storage_root: String,
}

// Function to generate random Ethereum account addresses
fn generate_eth_address(seed: usize) -> String {
    let mut rng: rand::rngs::StdRng = rand::SeedableRng::from_seed([seed as u8; 32]);
    let hex_chars: Vec<char> = "0123456789abcdef".chars().collect();
    let mut address = String::from("0x");
    for _ in 0..42 {
        let idx = rng.gen_range(0..16);
        address.push(hex_chars[idx]);
    }

    address
}

// Function to simulate creating a hashmap of Ethereum account addresses and balances
pub fn simulate_eth_accounts(seed: usize, num_accounts: usize) -> HashMap<String, EthAccount> {
    let mut rng: rand::rngs::StdRng = rand::SeedableRng::from_seed([seed as u8; 32]);
    let mut accounts = HashMap::new();
    for i in 0..num_accounts {
        let address = generate_eth_address(seed + i);
        let balance = rng.gen_range(100000..1000000);
        let mut eth_account = EthAccount {
            address: address.clone(),
            balance,
            storage_root: String::new(),
        };
        eth_account.storage_root = generate_account_storage_root(&eth_account);
        accounts.insert(address, eth_account);
    }

    accounts
}

// Function to generate the hash of the root node of the account storage trie
pub fn generate_account_storage_root(account_storage: &EthAccount) -> String {
    let account_storage_json = serde_json::to_string(account_storage).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(account_storage_json.as_bytes());
    encode(hasher.finalize())
}

// Function to generate the account state root hash
pub fn generate_account_state_root_hash(accounts: &HashMap<String, EthAccount>) -> String {
    let account_states: Vec<_> = accounts
        .iter()
        .map(|(_, account)| {
            let account_json = serde_json::to_string(account).unwrap();
            let mut hasher = Sha256::new();
            hasher.update(account_json.as_bytes());
            encode(hasher.finalize())
        })
        .collect();

    generate_merkle_root(&account_states)
}

pub fn generate_merkle_root(transaction_hashes: &[String]) -> String {
    let concatenated_hashes = transaction_hashes.concat();
    let mut hasher = Sha256::new();
    hasher.update(concatenated_hashes.as_bytes());
    encode(hasher.finalize())
}
