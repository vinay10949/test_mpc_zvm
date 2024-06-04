#![no_main]

use std::collections::{BTreeMap, HashMap};

use risc0_zkvm::guest::env;
use tx_prover::utils::{generate_account_state_root_hash, generate_account_storage_root, EthAccount, Transaction};

risc0_zkvm::guest::entry!(main);

fn main() {
  
    let transactions: Vec<Transaction> = env::read();

    let mut accounts: HashMap<String,EthAccount> = env::read();


    for txn in transactions.iter() {
        if let Some(from_account) = accounts.get_mut(&txn.from) {
            from_account.balance -= txn.amount;
            from_account.storage_root = generate_account_storage_root(from_account);
        }

        if let Some(to_account) = accounts.get_mut(&txn.to) {
            to_account.balance += txn.amount;
            to_account.storage_root = generate_account_storage_root(to_account);
        }
    }

    let sorted_accounts: BTreeMap<_, _> = accounts.clone().into_iter().collect();


    let account_state_root_hash = generate_account_state_root_hash(&sorted_accounts);
    println!("Account State Root Hash :{:?}", account_state_root_hash);
    
    env::commit(&(account_state_root_hash,accounts));
}
