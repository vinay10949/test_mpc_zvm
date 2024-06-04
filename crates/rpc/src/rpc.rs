use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use tx_utils::utils::Transaction;

#[rpc(client, server)]
#[async_trait::async_trait]
pub trait MpcRpc {
    #[method(name = "process_txn")]
    async fn process_txn(&self, transactions: Vec<Transaction>) -> Result<(), ErrorObjectOwned>;
}
