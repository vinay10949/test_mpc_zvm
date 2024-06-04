use std::collections::BTreeMap;
// Standard library imports
use std::fmt;

// Third-party library imports
use async_trait::async_trait;
use hex::encode;
use ice_frost::keys::DiffieHellmanPrivateKey;
use ice_frost::parameters::ThresholdParameters;
use ice_frost::{FromBytes, ToBytes};
use libp2p::{Multiaddr, PeerId};
use messages::message::{AppStateChangeMessage, Dkg, ParticipantDKG, SharedShares};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::rngs::OsRng;
use thiserror::*;

use ice_frost::testing::Secp256k1Sha256;

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ice_frost::dkg::{Coefficients, DistributedKeyGeneration, EncryptedSecretShare};
use ice_frost::sign::generate_commitment_share_lists;

// Project-specific imports
use messages::{
    actor_type::ActorType,
    message::{DkgEngineMessage, GossipEngineMessage, Message},
    NETWORK_TOPIC,
};

// Local crate imports
use crate::cast_message;

#[derive(Debug, Clone, Error)]
pub enum DKGEngineError {
    #[error("Error occurred in election engine: {0}")]
    Custom(String),
}

impl Default for DKGEngineError {
    fn default() -> Self {
        DKGEngineError::Custom("DKGEngine unable to acquire actor".to_string())
    }
}

#[derive(Clone, Debug)]
pub struct DKGEngineActor();

impl DKGEngineActor {
    pub fn new() -> Self {
        Self()
    }
}

#[derive(Debug, Default)]
pub struct DkgStateWrapper(Option<DKGEngineState>);

/// The `DKGEngineState` struct represents the state of a distributed key generation engine State.
pub struct DKGEngineState {
    peer_id: PeerId,
    participant: ParticipantDKG,
    coeff: Coefficients<Secp256k1Sha256>,
    threshold: u32,
    total_nodes: u32,
    other_participants: Vec<ParticipantDKG>,
    dh_sk: DiffieHellmanPrivateKey<Secp256k1Sha256>,
    my_encrypted_shares: Vec<EncryptedSecretShare<Secp256k1Sha256>>,
    my_state: Vec<DistributedKeyGeneration<ice_frost::dkg::RoundOne, Secp256k1Sha256>>,
}

impl fmt::Debug for DKGEngineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DKGEngineState")
            .field("threshold", &self.threshold)
            .field("size", &self.total_nodes)
            .finish()
    }
}

impl DKGEngineState {
    pub async fn dkg_init(
        peer_id: PeerId,
        idx: u128,
        elected_peers: Vec<(usize, PeerId, Multiaddr)>,
        threshold: usize,
    ) -> Result<Self, DKGEngineError> {
        let rng = OsRng;
        let params =
            ThresholdParameters::new(elected_peers.len() as u32, threshold as u32).unwrap();
        let (participant, coeff, dh_sk) =
            ParticipantDKG::new_dealer(params, idx as u32, rng).unwrap();

        Ok(Self {
            peer_id,
            participant: participant.clone(),
            coeff,
            threshold: threshold as u32,
            total_nodes: elected_peers.len() as u32,
            other_participants: vec![participant],
            dh_sk,
            my_encrypted_shares: vec![],
            my_state: vec![],
        })
    }
}

#[async_trait]
impl Actor for DKGEngineActor {
    type Msg = DkgEngineMessage;
    type State = DkgStateWrapper;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(DkgStateWrapper::default())
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DkgEngineMessage::Init(idx, peer_id, peers, threshold) => {
                handle_init(myself, state, idx, peer_id, peers, threshold).await
            }
            // DkgEngineMessage::Round1 => handle_round1(myself, state).await,
            DkgEngineMessage::InitRoundOutput(output) => {
                handle_init_round_output(myself, state, output)
            }
            DkgEngineMessage::StoreShares(shares) => {
                store_shares(myself, state, shares);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

async fn handle_init(
    _myself: ActorRef<DkgEngineMessage>,
    state: &mut DkgStateWrapper,
    idx: usize,
    peer_id: PeerId,
    peers: Vec<(usize, PeerId, Multiaddr)>,
    threshold: usize,
) -> Result<(), ActorProcessingErr> {
    println!("Initializing DKG engine");
    state.0 = Some(
        DKGEngineState::dkg_init(peer_id, idx as u128, peers, threshold)
            .await
            .map_err(ActorProcessingErr::from)?,
    );
    if let Some(dkg_state) = &state.0 {
        if let Ok(p_bytes) = dkg_state.participant.to_bytes() {
            cast_message!(
                ActorType::GossipEngine,
                GossipEngineMessage::Gossip(
                    Message::InitRoundOutput(p_bytes),
                    NETWORK_TOPIC.to_string(),
                )
            );
        };
    }
    Ok(())
}

fn handle_init_round_output(
    _myself: ActorRef<DkgEngineMessage>,
    state: &mut DkgStateWrapper,
    output: Vec<u8>,
) -> Result<(), ActorProcessingErr> {
    if let Some(state) = state.0.as_mut() {
        let rng = OsRng;

        if let Ok(p) = ParticipantDKG::from_bytes(&output) {
            state.other_participants.push(p);
            if state.other_participants.len() as u32 == state.total_nodes {
                let params = ThresholdParameters::new(state.total_nodes, state.threshold).unwrap();
                let (participant_state, _participant_lists) = Dkg::<_>::bootstrap(
                    params,
                    &state.dh_sk,
                    state.participant.index,
                    &state.coeff,
                    &state.other_participants,
                    rng,
                )
                .unwrap();
                state.my_state = vec![participant_state.clone()];
                let p_their_secret_shares: &std::collections::BTreeMap<
                    u32,
                    EncryptedSecretShare<Secp256k1Sha256>,
                > = participant_state.their_encrypted_secret_shares().unwrap();
                state.my_encrypted_shares.push(
                    p_their_secret_shares
                        .get(&state.participant.index)
                        .unwrap()
                        .clone(),
                );
                println!("Gossiping their shares");

                let mut new_shares = BTreeMap::new();
                for (idx, share) in p_their_secret_shares.iter() {
                    let mut bytes = Vec::with_capacity(share.compressed_size());
                    share.serialize_compressed(&mut bytes).unwrap();
                    new_shares.insert(*idx, bytes);
                }
                cast_message!(
                    ActorType::GossipEngine,
                    GossipEngineMessage::Gossip(
                        Message::GossipShare(new_shares),
                        NETWORK_TOPIC.to_string(),
                    )
                );
            }
        }
    }
    Ok(())
}

fn store_shares(
    _myself: ActorRef<DkgEngineMessage>,
    state: &mut DkgStateWrapper,
    shares: SharedShares,
) {
    if let Some(state) = state.0.as_mut() {
        if let Some(share) = shares.get(&state.participant.index) {
            let share = EncryptedSecretShare::deserialize_compressed(&share[..]).unwrap();
            state.my_encrypted_shares.push(share);
        }
        let rng = OsRng;
        if state.my_encrypted_shares.len() as u32 == state.total_nodes {
            let dkg_state = state.my_state.pop().unwrap();
            let (dkg_state, complaints) = dkg_state
                .to_round_two(&state.my_encrypted_shares, rng)
                .unwrap();
            assert!(complaints.is_empty());
            let (group_key, p_sk) = dkg_state.finish().unwrap();
            let (p_public_comshares, p_secret_comshares) =
                generate_commitment_share_lists(&mut OsRng, &p_sk, 1).unwrap();

            let p_shares = p_public_comshares.to_bytes().unwrap().clone();
            let g = group_key.to_bytes().unwrap();
            println!("Group Public key {:?}",encode(g.clone()));
            let p_k = p_sk.to_public().to_bytes().unwrap().clone();
            cast_message!(
                ActorType::AppStateEngine,
                AppStateChangeMessage::StoreKeyShares(
                    group_key,
                    p_sk,
                    p_secret_comshares,
                    p_public_comshares
                )
            );

            cast_message!(
                ActorType::GossipEngine,
                GossipEngineMessage::Gossip(
                    Message::GossipVerificationShares(state.participant.index, (p_shares, p_k, g)),
                    NETWORK_TOPIC.to_string(),
                )
            );
        }
    }
}
