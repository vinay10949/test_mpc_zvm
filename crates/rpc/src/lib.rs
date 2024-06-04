use actors::cast_message;
use async_trait::async_trait;
use jsonrpsee::types::ErrorObjectOwned;
use messages::{
    actor_type::ActorType,
    message::{GossipEngineMessage, Message},
    NETWORK_TOPIC,
};
use tx_utils::utils::Transaction;
pub mod rpc;
pub use rpc::*;

#[derive(Debug)]
pub struct RpcServerImpl;

impl RpcServerImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RpcServerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MpcRpcServer for RpcServerImpl {
    async fn process_txn(&self, transactions: Vec<Transaction>) -> Result<(), ErrorObjectOwned> {
        println!("Received RPC `process_txn` method");
        cast_message!(
            ActorType::GossipEngine,
            GossipEngineMessage::Gossip(
                Message::GossipTransactions(transactions),
                NETWORK_TOPIC.to_string(),
            )
        );

        Ok(())
    }
}
