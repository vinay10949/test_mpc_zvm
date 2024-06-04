use serde::{Deserialize, Serialize};
use std::fmt;
pub mod actor_type;
pub mod message;
pub const NETWORK_TOPIC: &str = "test-net";
pub const PROVER_TOPIC: &str = "prover-quorum";
pub const VERIFIER_TOPIC: &str = "verifier";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ElectionState {
    #[default]
    CollectingProposals,
    CollectedProposals,
    TriggerElection,
    QuorumElected,
}

impl fmt::Display for ElectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElectionState::CollectingProposals => write!(f, "CollectingProposals"),
            ElectionState::CollectedProposals => write!(f, "CollectedProposals"),
            ElectionState::QuorumElected => write!(f, "QuorumElected"),
            ElectionState::TriggerElection => write!(f, "TriggerElection"),
        }
    }
}
