// Third-party library imports
use async_trait::async_trait;
use libp2p::{identity::PublicKey, Multiaddr, PeerId};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use thiserror::*;

// Project-specific imports
use messages::{
    actor_type::ActorType,
    message::{
        AppStateChangeMessage, DkgEngineMessage, ElectionEngineMessage, GossipEngineMessage,
        Message,
    },
    ElectionState, NETWORK_TOPIC,
};
use network::utils::fetch_provers;

// Local crate imports
use crate::cast_message;

pub const THRESHOLD: f64 = 50.0;

/// A generic error type to propagate errors from this actor
/// and other actors that interact with it
#[derive(Debug, Clone, Error)]
pub enum ElectionEngineError {
    #[error("Error occurred in election engine: {0}")]
    Custom(String),
}

impl Default for ElectionEngineError {
    fn default() -> Self {
        ElectionEngineError::Custom("ElectionEngine unable to acquire actor".to_string())
    }
}

/// The actor struct for the Election Engine actor
#[derive(Clone, Debug, Default)]
pub struct ElectionEngineActor;

impl ElectionEngineActor {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone, Debug)]
pub struct ElectionEngineState {
    pub peer_id: PeerId,
    pub current_state: ElectionState,
    pub elected_provers: Vec<(PeerId, PublicKey, Multiaddr)>,
}

impl ElectionEngineState {
    pub async fn check_election_status(&mut self) -> Result<(), ElectionEngineError> {
        self.current_state = ElectionState::CollectedProposals;
        println!("Current Election State: {:?}", self.current_state);
        Ok(())
    }

    pub async fn conduct_election(&mut self) -> Result<(), ElectionEngineError> {
        self.current_state = ElectionState::TriggerElection;
        println!("Current Election State: {:?}", self.current_state);
        println!("Triggering Elections");
        self.current_state = ElectionState::QuorumElected;
        self.elected_provers = fetch_provers();
        println!("Election Results Declared");
        println!("Current Election State: {:?}", self.current_state);
        println!("List of Elected Provers");
        for (index, prover) in self.elected_provers.iter().enumerate() {
            println!("Position {:?}, Name :{:?}", index, prover);
        }
        Ok(())
    }

    fn calculate_threshold(n: usize, threshold_percentage: f64) -> usize {
        (n as f64 * threshold_percentage / 100.0).ceil() as usize
    }
}

#[async_trait]
impl Actor for ElectionEngineActor {
    type Msg = ElectionEngineMessage;
    type State = ElectionEngineState;
    type Arguments = PeerId;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(ElectionEngineState {
            peer_id: args,
            current_state: ElectionState::default(),
            elected_provers: Vec::default(),
        })
    }

    /// Handles different messages related to an election engine, triggering actions based on the current state.
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ElectionEngineMessage::CheckElectionStatus => handle_check_election_status(state).await,
            ElectionEngineMessage::TriggerElection => handle_trigger_election(state).await,
            ElectionEngineMessage::CheckElectedStatus(peer_ids) => {
                handle_check_elected_status(state, peer_ids).await
            }
        }
    }
}

async fn handle_check_election_status(
    state: &mut ElectionEngineState,
) -> Result<(), ActorProcessingErr> {
    let _ = state.check_election_status().await;
    if state.current_state == ElectionState::CollectedProposals {
        cast_message!(
            ActorType::ElectionEngine,
            ElectionEngineMessage::TriggerElection
        );
    }
    Ok(())
}

async fn handle_trigger_election(
    state: &mut ElectionEngineState,
) -> Result<(), ActorProcessingErr> {
    let _ = state.conduct_election().await;
    if state.current_state == ElectionState::QuorumElected {
        let peers = state
            .elected_provers
            .iter()
            .enumerate()
            .map(|(idx, (peer_id, _, multi_addr))| (idx + 1, *peer_id, multi_addr.clone()))
            .collect::<Vec<(usize, PeerId, Multiaddr)>>();

        cast_message!(
            ActorType::AppStateEngine,
            AppStateChangeMessage::UpdateElectedProvers(peers.clone())
        );

        let threshold = ElectionEngineState::calculate_threshold(peers.len(), THRESHOLD) as u32;
        cast_message!(
            ActorType::AppStateEngine,
            AppStateChangeMessage::UpdatedThreshold(peers.len() as u32, threshold)
        );

        cast_message!(
            ActorType::GossipEngine,
            GossipEngineMessage::Gossip(
                Message::ElectedProvers(peers.len() as u32, threshold, peers),
                NETWORK_TOPIC.to_string(),
            )
        );
    }
    Ok(())
}

async fn handle_check_elected_status(
    state: &mut ElectionEngineState,
    peer_ids: Vec<(usize, PeerId, Multiaddr)>,
) -> Result<(), ActorProcessingErr> {
    let is_elected = peer_ids
        .iter()
        .any(|(_, peer_id, _)| peer_id == &state.peer_id);

    let idx = peer_ids
        .iter()
        .find(|(_, peer_id, _)| peer_id == &state.peer_id)
        .map(|(idx, _, _)| *idx);

    if is_elected {
        println!("Elected for the Quorum, Index :{}", idx.unwrap().clone());
        let threshold = ElectionEngineState::calculate_threshold(peer_ids.len(), THRESHOLD);
        println!(
            "Proceeding for DKG with size :{} , threshold :{}",
            peer_ids.len(),
            threshold,
        );

        println!("Checking election status for Quorum");
        if let Some(idx) = idx {
            cast_message!(
                ActorType::DkgEngine,
                DkgEngineMessage::Init(
                    idx,
                    state.peer_id,
                    peer_ids.into_iter().map(|(a, b, c)| (a, b, c)).collect(),
                    threshold
                )
            );

            println!("Calling DKG INIT IDX:{:?}", idx);
            cast_message!(
                ActorType::AppStateEngine,
                AppStateChangeMessage::UpdateNodeIdx(idx as u32)
            );
        }
    } else {
        println!("Not Elected for the Quorum");
    }
    Ok(())
}
