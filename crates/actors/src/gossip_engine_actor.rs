// Standard library imports
use std::collections::{HashMap, HashSet};

// Third-party library imports
use async_trait::async_trait;
use ice_frost::{
    keys::{GroupVerifyingKey, IndividualVerifyingKey},
    sign::{PartialThresholdSignature, PublicCommitmentShareList},
    testing::Secp256k1Sha256,
    FromBytes,
};
use libp2p::{
    futures::StreamExt,
    gossipsub::{Event, IdentTopic},
    identity::Keypair,
    swarm::{Swarm, SwarmEvent},
    Multiaddr, PeerId,
};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use thiserror::*;
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};

// Project-specific imports
use messages::{
    actor_type::ActorType,
    message::{
        AppStateChangeMessage, DkgEngineMessage, ElectionEngineMessage, GossipEngineMessage,
        Message,
    },
    NETWORK_TOPIC,
};
use network::{
    behaviour::{MyBehaviour, MyBehaviourEvent},
    utils::NodeType,
};

// Local crate imports
use crate::{cast_message, ProcessMessageError};

#[derive(Debug, Clone, Error)]
pub enum GossipEngineError {
    #[error("Error occurred in gossip engine: {0}")]
    Custom(String),
}

impl Default for GossipEngineError {
    fn default() -> Self {
        GossipEngineError::Custom("GossipEngine unable to acquire actor".to_string())
    }
}

pub struct GossipEngineListener {
    swarm: Swarm<MyBehaviour>,
    _sender: UnboundedSender<MessageWithMultiaddr>,
    receiver: UnboundedReceiver<MessageWithMultiaddr>,
    topics_subscribed: HashMap<PeerId, HashSet<String>>,
    bootstrap_address: HashSet<PeerId>,
    node_type: NodeType,
    _relay_peer_id: PeerId,
    _relay_multi_addr: Multiaddr,
    polled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GossipEngineActor;

impl GossipEngineActor {
    pub fn new() -> Self {
        Self
    }
}

type MessageWithMultiaddr = (Message, String, Option<Vec<(PeerId, Multiaddr)>>);

impl GossipEngineListener {
    pub async fn new(
        keypair: Keypair,
        bootstrap_addresses: Option<Vec<(PeerId, Multiaddr)>>,
        port: String,
        sender: UnboundedSender<MessageWithMultiaddr>,
        receiver: UnboundedReceiver<MessageWithMultiaddr>,
        node_type: NodeType,
        _relay_peer_id: PeerId,
        _relay_multi_addr: Multiaddr,
    ) -> Result<Self, GossipEngineError> {
        let peer_id = keypair.public().to_peer_id();
        let bootstrap_addresses_cloned = bootstrap_addresses.clone();
        let swarm = network::network::init_swarm(Some(keypair), bootstrap_addresses.clone(), port)
            .await
            .map_err(|e| GossipEngineError::Custom(e.to_string()))?;

        let mut topics_subscribed = HashMap::new();
        topics_subscribed
            .entry(peer_id)
            .or_insert_with(HashSet::new)
            .insert(NETWORK_TOPIC.to_string());
        let mut bootstrap_address = HashSet::new();
        if let Some(bootstrap_address_list) = bootstrap_addresses_cloned {
            bootstrap_address.extend(bootstrap_address_list.iter().map(|(peer_id, _)| *peer_id));
        }

        Ok(Self {
            swarm,
            _sender: sender,
            receiver,
            topics_subscribed,
            bootstrap_address,
            node_type,
            _relay_peer_id,
            _relay_multi_addr,
            polled: false,
        })
    }

    fn check_bootstrap_topic(&self, topic: &str) -> bool {
        self.bootstrap_address.iter().any(|peer_id| {
            self.topics_subscribed
                .get(peer_id)
                .map_or(false, |topics| topics.contains(topic))
        })
    }

    /// function that represents the main loop for handling events in the gossip engine.
    pub async fn start(&mut self) {
        loop {
            select! {
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(Event::Message {
                        propagation_source: peer_id,
                        message_id: _,
                        message,
                    })) => {
                        let msg = Message::deserialize_from_bytes(&message.data);
                        match msg {
                            Ok(msg) => {
                                println!("Received Gossiped Message {:?}", msg.to_string());
                                if let Err(e) = self.process_message(msg) {
                                    println!(
                                        "Error processing message: {:?} from peer: {:?}, Error: {:?}",
                                        message.data,
                                        peer_id,
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                println!(
                                    "Received Invalid Message '{:?}' from peer: {:?}, Size: {}, error: {}",
                                    String::from_utf8_lossy(&message.data),
                                    peer_id,
                                    message.data.len(),
                                    e
                                );
                            }
                        }
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Local node is listening on {}", address);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(Event::Subscribed { topic, peer_id })) => {
                        println!("Peer_ID: {:?} Topic Subscribed: {:?}", peer_id, topic);
                        self.topics_subscribed
                            .entry(peer_id)
                            .or_default()
                            .insert(topic.to_string());
                        if self.node_type == NodeType::Relay
                            && self.check_bootstrap_topic(&topic.to_string())
                            && !self.polled
                        {
                            println!("Polling for Election Status");
                            cast_message!(
                                ActorType::ElectionEngine,
                                ElectionEngineMessage::CheckElectionStatus
                            );
                            self.polled = true;
                        }
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(Event::Unsubscribed { topic, peer_id })) => {
                        println!("Peer_ID: {:?} Topic Unsubscribed: {:?}", peer_id, topic);
                        if let Some(set) = self.topics_subscribed.get_mut(&peer_id) {
                            set.remove(&topic.to_string());
                            if set.is_empty() {
                                self.topics_subscribed.remove(&peer_id);
                            }
                        }
                    }
                    _ => {
                        println!("Received event: {:?}", event);
                    }
                },

                Some(msg) = self.receiver.recv() => match msg.0 {
                        Message::ElectedProvers(n,t,peer_ids) => {
                            let new_msg = Message::ElectedProvers(n,t,peer_ids.clone());
                            let serialized_msg = new_msg.serialize_to_bytes().unwrap();
                            println!("Gossiping Elected Provers to topic: {:?}, {:?}", peer_ids, msg.1);
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(IdentTopic::new(msg.1.clone()), serialized_msg.clone()) {
                                println!("Publish error: {:?}", e);
                            }
                        },
                        Message::InitRoundOutput(_)|
                        Message::GossipShare(_)|Message::GossipTransactions(_) => {
                            let new_msg = msg.0.clone();
                            let serialized_msg = new_msg.serialize_to_bytes().unwrap();
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(IdentTopic::new(msg.1.clone()), serialized_msg.clone()) {
                                println!("Publish error: {:?}", e);
                            }
                        },

                        Message::GossipVerificationShares(_,_) => {
                            let new_msg = msg.0.clone();
                            let serialized_msg = new_msg.serialize_to_bytes().unwrap();
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(IdentTopic::new(msg.1.clone()), serialized_msg.clone()) {
                                println!("Publish error: {:?}", e);
                            }
                        },
                        Message::BroadcastPartialSig(_,_,_) => {
                            let new_msg = msg.0.clone();
                            let serialized_msg = new_msg.serialize_to_bytes().unwrap();
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(IdentTopic::new(msg.1.clone()), serialized_msg.clone()) {
                                println!("Publish error: {:?}", e);
                            }
                        },

                        Message::Subscribe(topic) => {
                            let s = self.swarm.behaviour_mut().gossipsub.subscribe(&IdentTopic::new(topic.clone()));
                            println!("Subscription status to topic: {:?}: {:?}", topic, s);
                        }
                        _ => {
                          println!("Different message received: {:?}", msg);
                        }

                },
            }
        }
    }

    fn process_message(&self, message: Message) -> Result<(), ProcessMessageError> {
        match message {
            Message::ElectedProvers(n, t, peer_ids) => {
                cast_message!(
                    ActorType::ElectionEngine,
                    ElectionEngineMessage::CheckElectedStatus(peer_ids)
                );
                cast_message!(
                    ActorType::AppStateEngine,
                    AppStateChangeMessage::UpdatedThreshold(n, t)
                );
                Ok(())
            }
            Message::InitRoundOutput(values) => {
                cast_message!(
                    ActorType::DkgEngine,
                    DkgEngineMessage::InitRoundOutput(values)
                );
                Ok(())
            }
            Message::Subscribe(_) => todo!(),
            Message::Unsubscribe(_) => todo!(),
            Message::Other(data) => {
                println!("Recieved data from provers :{:?}", data);
                Ok(())
            }
            Message::GossipShare(shares) => {
                cast_message!(ActorType::DkgEngine, DkgEngineMessage::StoreShares(shares));
                Ok(())
            }
            Message::GossipVerificationShares(idx, (p_shares, p_k, g)) => {
                let p_shares = PublicCommitmentShareList::from_bytes(&p_shares).unwrap();
                let g = GroupVerifyingKey::from_bytes(&g).unwrap();
                let p_k = IndividualVerifyingKey::from_bytes(&p_k).unwrap();
                cast_message!(
                    ActorType::AppStateEngine,
                    AppStateChangeMessage::StoreVerificationShares(idx, (p_shares, p_k, g))
                );
                Ok(())
            }
            Message::GossipTransactions(transactions) => {
                cast_message!(
                    ActorType::AppStateEngine,
                    AppStateChangeMessage::ProcessTxn(transactions)
                );
                Ok(())
            }
            Message::BroadcastPartialSig(account_state_root_hash, receipt, signature) => {
                let signature = PartialThresholdSignature::from_bytes(&signature).unwrap();
                cast_message!(
                    ActorType::AppStateEngine,
                    AppStateChangeMessage::ProcessPartialSig(
                        account_state_root_hash,
                        receipt,
                        signature
                    )
                );
                Ok(())
            }
            _ => todo!(),
        }
    }
}

type SenderReceiver = (
    UnboundedSender<(Message, String, Option<Vec<(PeerId, Multiaddr)>>)>,
    UnboundedReceiver<(Message, String, Option<Vec<(PeerId, Multiaddr)>>)>,
);

#[async_trait]
impl Actor for GossipEngineActor {
    type Msg = GossipEngineMessage;
    type State = SenderReceiver;
    type Arguments = SenderReceiver;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(args)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            GossipEngineMessage::Gossip(message, topic) => {
                let _ = state.0.send((message.clone(), topic.clone(), None));
            }
            GossipEngineMessage::Subscribe(topic) => {
                let _ = state
                    .0
                    .send((Message::Subscribe(topic), "".to_string(), None));
            }
            GossipEngineMessage::Forward(message, peer_ids) => {
                let _ = state
                    .0
                    .send((message.clone(), String::from(""), Some(peer_ids)));
            }
            _ => {}
        }
        Ok(())
    }
}
