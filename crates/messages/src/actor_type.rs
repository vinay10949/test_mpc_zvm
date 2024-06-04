use serde::{Deserialize, Serialize};
use tokio::time::Duration;

pub const TIMEOUT_DURATION: Duration = tokio::time::Duration::from_millis(200);

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub enum ActorType {
    GossipEngine,
    ElectionEngine,
    Relay,
    DkgEngine,
    AppStateEngine,
    Rpc,
}

impl ToString for ActorType {
    fn to_string(&self) -> String {
        match self {
            ActorType::GossipEngine => "GossipEngine".to_string(),
            ActorType::ElectionEngine => "ElectionEngine".to_string(),
            ActorType::Relay => "Relay".to_string(),
            ActorType::DkgEngine => "DkgEngine".to_string(),
            ActorType::Rpc => "RPC".to_string(),
            ActorType::AppStateEngine => "AppStateEngine".to_string(),
        }
    }
}
