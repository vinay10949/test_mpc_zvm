use ice_frost::dkg::{DistributedKeyGeneration, Participant};
use ice_frost::keys::{GroupVerifyingKey, IndividualSigningKey, IndividualVerifyingKey};
use ice_frost::sign::{
    PartialThresholdSignature, PublicCommitmentShareList, SecretCommitmentShareList,
};
use libp2p::{Multiaddr, PeerId};
use ractor_cluster::RactorMessage;
use risc0_zkvm::InnerReceipt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tx_utils::utils::Transaction;

use ice_frost::testing::Secp256k1Sha256;

pub type ParticipantDKG = Participant<Secp256k1Sha256>;
pub type Dkg<T> = DistributedKeyGeneration<T, Secp256k1Sha256>;

pub type PublicCommShareList = PublicCommitmentShareList<Secp256k1Sha256>;
pub type SecretCommShareList = SecretCommitmentShareList<Secp256k1Sha256>;
pub type SharedShares = BTreeMap<u32, Vec<u8>>;

/// DKG Engine Messages
#[derive(Debug, RactorMessage)]
pub enum DkgEngineMessage {
    Init(usize, PeerId, Vec<(usize, PeerId, Multiaddr)>, usize),
    Round1,
    StoreShares(SharedShares),
    InitRoundOutput(Vec<u8>),
    Round2,
    Round3,
}

/// Election Engine Messages
#[derive(Debug, RactorMessage)]
pub enum ElectionEngineMessage {
    CheckElectionStatus,
    TriggerElection,
    CheckElectedStatus(Vec<(usize, PeerId, Multiaddr)>),
}

/// App State Change Messages
#[derive(Debug, RactorMessage)]
pub enum AppStateChangeMessage {
    UpdateElectedProvers(Vec<(usize, PeerId, Multiaddr)>),
    UpdatedThreshold(u32, u32),
    UpdateNodeIdx(u32),
    StoreKeyShares(
        GroupVerifyingKey<Secp256k1Sha256>,
        IndividualSigningKey<Secp256k1Sha256>,
        SecretCommitmentShareList<Secp256k1Sha256>,
        PublicCommitmentShareList<Secp256k1Sha256>,
    ),
    StoreVerificationShares(
        u32,
        (
            PublicCommitmentShareList<Secp256k1Sha256>,
            IndividualVerifyingKey<Secp256k1Sha256>,
            GroupVerifyingKey<Secp256k1Sha256>,
        ),
    ),
    ProcessTxn(Vec<Transaction>),
    ProcessPartialSig(
        String,
        InnerReceipt,
        PartialThresholdSignature<Secp256k1Sha256>,
    ),
}

pub type Topic = String;

/// General Message
#[derive(Debug, Clone, Serialize, Deserialize, RactorMessage)]
pub enum Message {
    ElectedProvers(u32, u32, Vec<(usize, PeerId, Multiaddr)>),
    BroadcastPartialSig(String, InnerReceipt, Vec<u8>),
    InitRoundOutput(Vec<u8>),
    GossipShare(BTreeMap<u32, Vec<u8>>),
    Subscribe(Topic),
    Unsubscribe(Topic),
    Message(String),
    Other(String),
    GossipVerificationShares(u32, (Vec<u8>, Vec<u8>, Vec<u8>)),
    GossipTransactions(Vec<Transaction>),
}

impl ToString for Message {
    fn to_string(&self) -> String {
        match self {
            Message::ElectedProvers(_,_,_) => "ElectedProvers".to_string(),
            Message::BroadcastPartialSig(_,_,_) => "BroadcastPartialSig".to_string(),
            Message::InitRoundOutput(_) => "InitRoundOutput".to_string(),
            Message::GossipShare (_)=> "GossipShare".to_string(),
            Message::Subscribe(_) => "Subscribe".to_string(),
            Message::Unsubscribe(_) => "Unsubscribe".to_string(),
            Message::Message(_) => "Message".to_string(),
            Message::Other(_) => "Other".to_string(),
            Message::GossipVerificationShares(_,_) => "GossipVerificationShares".to_string(),
            Message::GossipTransactions(_) => "GossipTransactions".to_string(),
        }
    }
}
impl Message {
    pub fn serialize_to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn deserialize_from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// Gossip Engine Messages
#[derive(Debug, RactorMessage)]
pub enum GossipEngineMessage {
    Gossip(Message, Topic),
    ForwardToRelay(PeerId),
    Forward(Message, Vec<(PeerId, Multiaddr)>),
    Subscribe(Topic),
    Unsubscribe(Topic),
}
